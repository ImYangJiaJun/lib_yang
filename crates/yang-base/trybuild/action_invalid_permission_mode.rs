use yang_base::Action;

#[derive(Action)]
#[action(name = "broken", permission_mode = "Any")]
struct BrokenAction;

fn main() {}
