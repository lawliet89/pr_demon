use ::rest;
use hyper;

#[derive(Clone)]
pub struct GooGl {
    key: String
}

impl GooGl {
    pub fn new(key: &str) -> GooGl {
        GooGl {
            key: key.to_owned()
        }
    }
}

impl ::Shortener for GooGl {
    fn shorten(&self, url: &str) -> Result<String, String> {
        Ok("".to_owned())
    }
}
