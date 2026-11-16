use super::{Input, Installer, common};
use crate::{error::Result, operation::Operation, registry::Kind};

pub struct AppImage;

impl Installer for AppImage {
    fn plan(&self, input: &Input) -> Result<Vec<Operation>> {
        let file = common::canonical(input.file)?;
        let identity = common::identify(&file, input.name)?;
        let paths = input.paths;
        common::ensure_free(paths, &identity.name)?;
        let dest = paths.app_dir(&identity.name);
        let target = dest.join(common::file_name(&file));
        let link = paths.bin_dir().join(&identity.name);
        common::absent(&link)?;
        let desktop = common::desktop_entry(paths, &identity, &target, None)?;
        let desktop_path = paths.desktop_file(&identity.name);
        let record = common::register(
            &identity,
            Kind::AppImage,
            common::file_name(&file),
            vec![desktop_path, link.clone(), dest],
        );
        Ok(vec![
            Operation::Copy {
                from: file,
                to: target.clone(),
                mode: 0o755,
            },
            Operation::Link { target, link },
            desktop,
            record,
        ])
    }
}
