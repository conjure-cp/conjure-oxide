//! Utility functions to collect Essence models.

use std::path::PathBuf;

use walkdir::WalkDir;

use crate::{FileSource, Model, ModelAndParams, ParamFile};

use glob::glob;

/// Sources models and param files from `dir` and all immediate subdirectories.
///
/// - Only models with the extensions .eprime and .essence are included.
///
/// - Parameter files for ame>.essence should begin with <name>, and have the extension .param.
///   They also must be in the same directory as the model.
///
///
/// # Returns
///
/// - If dir does not exist or is a file that is not a model, an empty vector is returned.
pub fn models_from_directory_tree(dir: &PathBuf) -> Result<Vec<ModelAndParams>, walkdir::Error> {
    let mut output = vec![];

    // only use walkdir to traverse directories not files inside of them. This is necessary to look
    // at all the files in a specific directory at the same time and match models to param files only
    // if they are in the same directory.
    for entry in WalkDir::new(dir)
        .follow_links(true)
        .into_iter()
        .filter_entry(|x| x.file_type().is_dir())
    {
        let entry = entry?;
        let path = entry.path();
        let path_str = path.to_str().unwrap();

        let models = glob(format!("{path_str}/*.eprime").as_str())
            .unwrap()
            .chain(glob(format!("{path_str}/*.essence").as_str()).unwrap());

        for model_path in models {
            let model_path = model_path.unwrap();
            if !model_path.is_file() {
                continue;
            }

            let model_name = model_path
                .file_stem()
                .expect("path to have a file stem as it is an ordinary file")
                .to_str()
                .unwrap();

            let model = Model {
                name: String::from(model_name),
                source: FileSource::File(model_path.clone()),
            };

            // find param files for this model
            let param_file_paths =
                glob(format!("{path_str}/{model_name}*.param").as_str()).unwrap();
            let mut params = Vec::<ParamFile>::new();

            for param_file_path in param_file_paths {
                let param_file_path = param_file_path.unwrap();
                if !param_file_path.is_file() {
                    continue;
                }

                let param_file_name = param_file_path
                    .file_stem()
                    .expect("path to have a file stem as it is an ordinary file")
                    .to_str()
                    .unwrap();

                params.push(ParamFile {
                    name: String::from(param_file_name),
                    source: FileSource::File(param_file_path.clone()),
                });
            }

            // convert param_files from Vec<ParamFile> to an Option<Vec<ParamFile>>, as required
            // for ModelAndParams. --> If no param files were found, set to None.
            let params: Option<Vec<ParamFile>> = if params.is_empty() {
                None
            } else {
                Some(params)
            };

            output.push(ModelAndParams { model, params })
        }
    }

    Ok(output)
}
