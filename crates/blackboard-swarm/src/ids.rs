use serde::{Deserialize, Serialize};

macro_rules! id_newtype {
    ($name:ident) => {
        #[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq, Hash)]
        pub struct $name(pub String);

        impl $name {
            pub fn new() -> Self {
                Self(crate::id_gen::new_id(stringify!($name)))
            }

            pub fn from_str(value: impl Into<String>) -> Self {
                Self(value.into())
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl From<String> for $name {
            fn from(value: String) -> Self {
                Self(value)
            }
        }

        impl From<&str> for $name {
            fn from(value: &str) -> Self {
                Self(value.to_string())
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.0)
            }
        }
    };
}

id_newtype!(RunId);
id_newtype!(DocumentId);
id_newtype!(EntryId);
id_newtype!(SignalId);
id_newtype!(ObligationId);
