use std::fmt;

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// The id does not refer to a live row.
    NotFound { relation: &'static str },
    /// A unique index already has a row with this key.
    Unique { relation: &'static str, index: &'static str },
    /// A foreign key column refers to a row that does not exist, or to a relation the database does not have.
    ForeignKey { relation: &'static str, column: &'static str, references: &'static str },
    /// A delete was refused because a row that would survive it still refers to a deleted row.
    Restrict { relation: &'static str, column: &'static str, references: &'static str },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::NotFound { relation } => write!(f, "no such {relation}"),
            Error::Unique { relation, index } => {
                write!(f, "unique index `{index}` on {relation} already has this key")
            }
            Error::ForeignKey { relation, column, references } => {
                write!(f, "{relation}.{column} refers to a {references} that does not exist")
            }
            Error::Restrict { relation, column, references } => {
                write!(f, "cannot delete {references}: {relation}.{column} still refers to it")
            }
        }
    }
}

impl std::error::Error for Error {}
