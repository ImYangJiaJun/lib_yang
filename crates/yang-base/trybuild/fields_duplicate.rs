use yang_base::definition::Str;

fn main() {
    let _ = yang_base::fields! {
        username => Str::new(),
        username => Str::new(),
    };
}
