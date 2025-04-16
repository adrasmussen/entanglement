use std::{
    fs::{canonicalize, create_dir, exists, read, remove_file, write},
    path::PathBuf,
    sync::Arc,
};

use anyhow;
use rand::random;

use common::config::ESConfig;

pub fn create_temp_file(dir: &PathBuf) -> anyhow::Result<()> {
    // needed to be completely unambiguous which directory we are checking
    if !dir.is_absolute() {
        return Err(anyhow::Error::msg(
            "must pass absolute path to create_temp_file",
        ));
    }

    if !(&canonicalize(&dir)? == dir) {
        return Err(anyhow::Error::msg(
            "must pass canonical path to create_temp_file",
        ));
    }

    // this ensures that we create a new file
    let mut filename = dir.join(random::<i64>().to_string());
    let mut count = 0;

    while exists(&filename)? {
        filename = dir.join(random::<i64>().to_string());

        if count < 10 {
            count += 1;
        } else {
            return Err(anyhow::Error::msg(format!(
                "create_temp_file failed to find unique filename ten times for directory {dir:?}"
            )));
        }
    }

    // mock data to make sure that we can read any file we create
    let data = random::<i64>().to_ne_bytes();

    write(&filename, &data)?;

    if read(&filename)? != data {
        return Err(anyhow::Error::msg(format!(
            "data readback failed on {filename:?}"
        )));
    }

    remove_file(&filename)?;

    Ok(())
}

pub fn subdir_exists(config: &Arc<ESConfig>, subdir: &str) -> anyhow::Result<()> {
    let subdir = PathBuf::from(subdir);

    if subdir.is_absolute() {
        return Err(anyhow::Error::msg(format!(
            "INTERNAL ERROR: constant {subdir:?} is an absolute path",
        )));
    }

    let full_subdir = config.fs.media_srvdir.join(subdir);

    if !exists(&full_subdir)? {
        create_dir(&full_subdir)?;
    }

    create_temp_file(&full_subdir)
}
