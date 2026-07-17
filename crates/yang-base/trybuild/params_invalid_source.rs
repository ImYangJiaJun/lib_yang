use yang_base::definition::Str;

yang_base::params! {
    InvalidInput {
        #[param(source = cookie)]
        token: Str::new().require(true),
    }
}

fn main() {}
