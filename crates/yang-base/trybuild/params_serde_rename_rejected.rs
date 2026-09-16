use yang_base::definition::Str;

yang_base::params! {
    pub Renamed {
        #[param(source = query)]
        #[serde(rename = "userId")]
        user_id: Str::new().require(true),
    }
}

fn main() {}
