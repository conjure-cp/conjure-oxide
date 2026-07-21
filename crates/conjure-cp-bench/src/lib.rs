use std::path::PathBuf;

pub mod executor;
pub mod mode;
pub mod model_sources;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Model {
    pub name: String,
    pub source: FileSource,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModelAndParams {
    pub model: Model,
    pub params: Option<Vec<ParamFile>>,
}

impl ModelAndParams {
    /// Constructs an iterator of (model,param) combinations.
    ///
    /// If this model does not require parameter files, a single iteration (model,None) is returned.
    pub fn iter(&self) -> impl Iterator<Item = (&Model, Option<&ParamFile>)> {
        ModelAndParamsIter::new(&self.model, &self.params)
    }
}

struct ModelAndParamsIter<'a> {
    model: &'a Model,
    params: &'a [ParamFile],
    i: usize,
    no_params: bool,
}

impl<'a> ModelAndParamsIter<'a> {
    fn new(model: &'a Model, params: &'a Option<Vec<ParamFile>>) -> ModelAndParamsIter<'a> {
        ModelAndParamsIter {
            model,
            params: params.as_deref().unwrap_or_default(),
            i: 0,
            no_params: params.is_none(),
        }
    }
}

impl<'a> Iterator for ModelAndParamsIter<'a> {
    type Item = (&'a Model, Option<&'a ParamFile>);

    fn next(&mut self) -> Option<Self::Item> {
        let next_params = self.params.get(self.i);

        // if no params and first run, return (model,None).
        if self.i == 0 && self.no_params {
            self.i += 1;
            Some((self.model, None))
        } else {
            self.i += 1;
            next_params.map(|p| (self.model, Some(p)))
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParamFile {
    pub name: String,
    pub source: FileSource,
}

/// The source of an essence file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FileSource {
    Text(String),
    File(PathBuf),
}
