const MAX_PROGRAM_HEADERS: usize = 512;
const MAX_DYNAMIC_ENTRIES: usize = 4096;
const MAX_STRING_LEN: usize = 4096;

const PT_LOAD: u32 = 1;
const PT_DYNAMIC: u32 = 2;
const DT_NULL: i64 = 0;
const DT_NEEDED: i64 = 1;
const DT_STRTAB: i64 = 5;
const DT_SONAME: i64 = 14;
const DT_RPATH: i64 = 15;
const DT_RUNPATH: i64 = 29;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Class {
    Bits32,
    Bits64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Elf {
    pub class: Class,
    pub machine: u16,
    pub needed: Vec<String>,
    pub soname: Option<String>,
    pub rpath: Vec<String>,
    pub runpath: Vec<String>,
}

struct Reader<'a> {
    data: &'a [u8],
    little: bool,
    wide: bool,
}

impl Reader<'_> {
    fn bytes<const N: usize>(&self, position: usize) -> Option<[u8; N]> {
        let end = position.checked_add(N)?;
        self.data.get(position..end)?.try_into().ok()
    }

    fn u16_at(&self, position: usize) -> Option<u16> {
        let raw = self.bytes::<2>(position)?;
        Some(if self.little {
            u16::from_le_bytes(raw)
        } else {
            u16::from_be_bytes(raw)
        })
    }

    fn u32_at(&self, position: usize) -> Option<u32> {
        let raw = self.bytes::<4>(position)?;
        Some(if self.little {
            u32::from_le_bytes(raw)
        } else {
            u32::from_be_bytes(raw)
        })
    }

    fn u64_at(&self, position: usize) -> Option<u64> {
        let raw = self.bytes::<8>(position)?;
        Some(if self.little {
            u64::from_le_bytes(raw)
        } else {
            u64::from_be_bytes(raw)
        })
    }

    fn word_at(&self, position: usize) -> Option<u64> {
        if self.wide {
            self.u64_at(position)
        } else {
            self.u32_at(position).map(u64::from)
        }
    }

    fn signed_word_at(&self, position: usize) -> Option<i64> {
        if self.wide {
            self.u64_at(position).map(|value| value as i64)
        } else {
            self.u32_at(position).map(|value| i64::from(value as i32))
        }
    }

    fn c_string(&self, position: u64) -> Option<String> {
        let start = usize::try_from(position).ok()?;
        let tail = self.data.get(start..)?;
        let length = tail
            .iter()
            .take(MAX_STRING_LEN)
            .position(|byte| *byte == 0)?;
        Some(String::from_utf8_lossy(&tail[..length]).into_owned())
    }
}

struct Segment {
    offset: u64,
    vaddr: u64,
    size: u64,
}

pub fn parse(data: &[u8]) -> Option<Elf> {
    if data.get(0..4)? != b"\x7fELF" {
        return None;
    }
    let wide = match data.get(4)? {
        1 => false,
        2 => true,
        _ => return None,
    };
    let little = match data.get(5)? {
        1 => true,
        2 => false,
        _ => return None,
    };
    let reader = Reader { data, little, wide };
    let machine = reader.u16_at(18)?;
    let (header_offset, entry_size, count) = if wide {
        (reader.u64_at(32)?, reader.u16_at(54)?, reader.u16_at(56)?)
    } else {
        (
            u64::from(reader.u32_at(28)?),
            reader.u16_at(42)?,
            reader.u16_at(44)?,
        )
    };
    let mut elf = Elf {
        class: if wide { Class::Bits64 } else { Class::Bits32 },
        machine,
        needed: Vec::new(),
        soname: None,
        rpath: Vec::new(),
        runpath: Vec::new(),
    };
    let (loads, dynamic) = segments(
        &reader,
        header_offset,
        usize::from(entry_size),
        usize::from(count),
    )?;
    if let Some(dynamic) = dynamic {
        read_dynamic(&reader, &dynamic, &loads, &mut elf);
    }
    Some(elf)
}

fn segments(
    reader: &Reader,
    header_offset: u64,
    entry_size: usize,
    count: usize,
) -> Option<(Vec<Segment>, Option<Segment>)> {
    let mut loads = Vec::new();
    let mut dynamic = None;
    if count == 0 {
        return Some((loads, dynamic));
    }
    let minimum = if reader.wide { 56 } else { 32 };
    if entry_size < minimum {
        return None;
    }
    let base = usize::try_from(header_offset).ok()?;
    for index in 0..count.min(MAX_PROGRAM_HEADERS) {
        let at = base.checked_add(index.checked_mul(entry_size)?)?;
        let kind = reader.u32_at(at)?;
        let segment = if reader.wide {
            Segment {
                offset: reader.u64_at(at + 8)?,
                vaddr: reader.u64_at(at + 16)?,
                size: reader.u64_at(at + 32)?,
            }
        } else {
            Segment {
                offset: u64::from(reader.u32_at(at + 4)?),
                vaddr: u64::from(reader.u32_at(at + 8)?),
                size: u64::from(reader.u32_at(at + 16)?),
            }
        };
        match kind {
            PT_LOAD => loads.push(segment),
            PT_DYNAMIC if dynamic.is_none() => dynamic = Some(segment),
            _ => {}
        }
    }
    Some((loads, dynamic))
}

fn read_dynamic(reader: &Reader, dynamic: &Segment, loads: &[Segment], elf: &mut Elf) {
    let entry = if reader.wide { 16 } else { 8 };
    let word = entry / 2;
    let Ok(start) = usize::try_from(dynamic.offset) else {
        return;
    };
    let capacity = usize::try_from(dynamic.size / entry as u64).unwrap_or(0);
    let mut strtab = None;
    let mut needed = Vec::new();
    let (mut soname, mut rpath, mut runpath) = (None, None, None);
    for index in 0..capacity.min(MAX_DYNAMIC_ENTRIES) {
        let Some(at) = start.checked_add(index * entry) else {
            break;
        };
        let (Some(tag), Some(value)) = (reader.signed_word_at(at), reader.word_at(at + word))
        else {
            break;
        };
        match tag {
            DT_NULL => break,
            DT_NEEDED => needed.push(value),
            DT_STRTAB => strtab = Some(value),
            DT_SONAME => soname = Some(value),
            DT_RPATH => rpath = Some(value),
            DT_RUNPATH => runpath = Some(value),
            _ => {}
        }
    }
    let Some(table) = strtab.and_then(|address| file_offset(loads, address)) else {
        return;
    };
    let string_at = |offset: u64| reader.c_string(table.checked_add(offset)?);
    elf.needed = needed.into_iter().filter_map(string_at).collect();
    elf.soname = soname.and_then(string_at);
    elf.rpath = rpath.and_then(string_at).map_or_else(Vec::new, split_paths);
    elf.runpath = runpath
        .and_then(string_at)
        .map_or_else(Vec::new, split_paths);
}

fn file_offset(loads: &[Segment], address: u64) -> Option<u64> {
    loads
        .iter()
        .find(|segment| {
            address >= segment.vaddr
                && segment
                    .vaddr
                    .checked_add(segment.size)
                    .is_some_and(|end| address < end)
        })
        .and_then(|segment| segment.offset.checked_add(address - segment.vaddr))
}

fn split_paths(list: String) -> Vec<String> {
    list.split(':')
        .filter(|part| !part.is_empty())
        .map(str::to_string)
        .collect()
}

#[cfg(any(test, feature = "testing"))]
pub mod testing {
    pub struct Spec<'a> {
        pub wide: bool,
        pub little: bool,
        pub machine: u16,
        pub needed: &'a [&'a str],
        pub soname: Option<&'a str>,
        pub runpath: Option<&'a str>,
        pub rpath: Option<&'a str>,
    }

    impl Default for Spec<'_> {
        fn default() -> Self {
            Spec {
                wide: true,
                little: true,
                machine: 62,
                needed: &[],
                soname: None,
                runpath: None,
                rpath: None,
            }
        }
    }

    struct Out {
        bytes: Vec<u8>,
        little: bool,
    }

    impl Out {
        fn put(&mut self, value: u64, size: usize) {
            let raw = value.to_le_bytes();
            let slice = &raw[..size];
            if self.little {
                self.bytes.extend_from_slice(slice);
            } else {
                self.bytes.extend(slice.iter().rev());
            }
        }
    }

    pub fn build(spec: &Spec) -> Vec<u8> {
        let word = if spec.wide { 8 } else { 4 };
        let header = if spec.wide { 64 } else { 52 };
        let program_header = if spec.wide { 56 } else { 32 };
        let dynamic_offset = header + 2 * program_header;
        let mut strings = vec![0u8];
        let mut add = |text: &str| {
            let at = strings.len() as u64;
            strings.extend_from_slice(text.as_bytes());
            strings.push(0);
            at
        };
        let needed: Vec<u64> = spec.needed.iter().map(|name| add(name)).collect();
        let soname = spec.soname.map(&mut add);
        let runpath = spec.runpath.map(&mut add);
        let rpath = spec.rpath.map(&mut add);
        let mut entries: Vec<(i64, u64)> = needed.into_iter().map(|at| (1, at)).collect();
        entries.extend(soname.map(|at| (14, at)));
        entries.extend(runpath.map(|at| (29, at)));
        entries.extend(rpath.map(|at| (15, at)));
        let strtab_address = 0x4000u64;
        entries.push((5, strtab_address));
        entries.push((0, 0));
        let dynamic_size = entries.len() * word * 2;
        let strtab_offset = dynamic_offset + dynamic_size;
        let total = strtab_offset + strings.len();
        let mut out = Out {
            bytes: Vec::new(),
            little: spec.little,
        };
        out.bytes.extend_from_slice(b"\x7fELF");
        out.bytes.push(if spec.wide { 2 } else { 1 });
        out.bytes.push(if spec.little { 1 } else { 2 });
        out.bytes.push(1);
        out.bytes.extend_from_slice(&[0; 9]);
        out.put(3, 2);
        out.put(u64::from(spec.machine), 2);
        out.put(1, 4);
        out.put(0, word);
        out.put(header as u64, word);
        out.put(0, word);
        out.put(0, 4);
        out.put(header as u64, 2);
        out.put(program_header as u64, 2);
        out.put(2, 2);
        out.put(0, 6);
        let segment = |out: &mut Out, kind: u64, offset: usize, vaddr: u64, size: usize| {
            if spec.wide {
                out.put(kind, 4);
                out.put(0, 4);
                out.put(offset as u64, 8);
                out.put(vaddr, 8);
                out.put(vaddr, 8);
                out.put(size as u64, 8);
                out.put(size as u64, 8);
                out.put(8, 8);
            } else {
                out.put(kind, 4);
                out.put(offset as u64, 4);
                out.put(vaddr, 4);
                out.put(vaddr, 4);
                out.put(size as u64, 4);
                out.put(size as u64, 4);
                out.put(0, 4);
                out.put(8, 4);
            }
        };
        segment(&mut out, 1, strtab_offset, strtab_address, strings.len());
        segment(&mut out, 2, dynamic_offset, 0x8000, dynamic_size);
        for (tag, value) in &entries {
            out.put(*tag as u64, word);
            out.put(*value, word);
        }
        out.bytes.extend_from_slice(&strings);
        assert_eq!(out.bytes.len(), total);
        out.bytes
    }
}

#[cfg(test)]
mod tests {
    use super::testing::{Spec, build};
    use super::*;

    #[test]
    fn reads_needed_soname_and_paths_from_a_64_bit_little_endian_file() {
        let data = build(&Spec {
            needed: &["libc.so.6", "libcrypt.so.1"],
            soname: Some("libfoo.so.2"),
            runpath: Some("$ORIGIN/../lib:/opt/x"),
            rpath: Some("$ORIGIN"),
            ..Spec::default()
        });
        let elf = parse(&data).unwrap();
        assert_eq!(elf.class, Class::Bits64);
        assert_eq!(elf.machine, 62);
        assert_eq!(elf.needed, ["libc.so.6", "libcrypt.so.1"]);
        assert_eq!(elf.soname.as_deref(), Some("libfoo.so.2"));
        assert_eq!(elf.runpath, ["$ORIGIN/../lib", "/opt/x"]);
        assert_eq!(elf.rpath, ["$ORIGIN"]);
    }

    #[test]
    fn all_four_class_and_endianness_combinations() {
        for (wide, little) in [(true, true), (true, false), (false, true), (false, false)] {
            let data = build(&Spec {
                wide,
                little,
                machine: 183,
                needed: &["liba.so.1", "libb.so.2"],
                ..Spec::default()
            });
            let elf = parse(&data).unwrap_or_else(|| panic!("wide={wide} little={little}"));
            assert_eq!(
                elf.needed,
                ["liba.so.1", "libb.so.2"],
                "wide={wide} little={little}"
            );
            assert_eq!(elf.machine, 183);
            assert_eq!(elf.class, if wide { Class::Bits64 } else { Class::Bits32 });
        }
    }

    #[test]
    fn a_file_without_a_dynamic_section_is_static() {
        let mut data = build(&Spec::default());
        data[56] = 0;
        data[57] = 0;
        let elf = parse(&data).unwrap();
        assert!(elf.needed.is_empty());
    }

    #[test]
    fn things_that_are_not_elf_are_rejected() {
        assert_eq!(parse(b""), None);
        assert_eq!(parse(b"#!/bin/sh\necho hi\n"), None);
        assert_eq!(parse(b"\x7fELF"), None);
        let mut bad_class = build(&Spec::default());
        bad_class[4] = 9;
        assert_eq!(parse(&bad_class), None);
    }

    #[test]
    fn truncated_and_corrupted_input_never_panics() {
        let original = build(&Spec {
            needed: &["libc.so.6", "libm.so.6"],
            runpath: Some("$ORIGIN"),
            ..Spec::default()
        });
        for length in 0..original.len() {
            let _ = parse(&original[..length]);
        }
        let mut seed = 0x1234_5678_9abc_def0u64;
        for _ in 0..20_000 {
            let mut copy = original.clone();
            for _ in 0..3 {
                seed = seed
                    .wrapping_mul(6364136223846793005)
                    .wrapping_add(1442695040888963407);
                let at = (seed >> 33) as usize % copy.len();
                copy[at] = (seed >> 20) as u8;
            }
            let _ = parse(&copy);
        }
    }

    #[test]
    fn huge_counts_and_offsets_do_not_overflow() {
        let mut data = build(&Spec::default());
        data[32..40].copy_from_slice(&u64::MAX.to_le_bytes());
        let _ = parse(&data);
        let mut data = build(&Spec::default());
        data[56..58].copy_from_slice(&u16::MAX.to_le_bytes());
        let _ = parse(&data);
    }

    #[test]
    fn this_very_test_binary_parses() {
        let data = std::fs::read(std::env::current_exe().unwrap()).unwrap();
        let elf = parse(&data).expect("the test executable is an ELF file");
        assert_ne!(elf.machine, 0);
        if !cfg!(target_env = "musl") {
            assert!(
                elf.needed.iter().any(|name| name.starts_with("libc.so")),
                "{:?}",
                elf.needed
            );
        }
    }
}
