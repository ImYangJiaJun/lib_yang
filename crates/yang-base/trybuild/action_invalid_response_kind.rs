use yang_base::Action;

#[derive(Action)]
#[action(name = "broken", response_kind = "stream")]
struct BrokenAction;

fn main() {}
