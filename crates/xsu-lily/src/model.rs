/// General API errors
#[derive(Debug)]
pub enum LilyError {
    MustBeUnique,
    NotAllowed,
    ValueError,
    NotFound,
    Other,
}

impl LilyError {
    pub fn to_string(&self) -> String {
        use LilyError::*;
        match self {
            MustBeUnique => String::from("One of the given values must be unique."),
            NotAllowed => String::from("You are not allowed to access this resource."),
            ValueError => String::from("One of the field values given is invalid."),
            NotFound => String::from("No asset with this ID could be found."),
            _ => String::from("An unspecified error has occured"),
        }
    }
}
