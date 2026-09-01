fn main() {
    use std::mem::size_of;
    println!("UiNode          {}", size_of::<dewey::ontology::UiNode>());
    println!("Properties      {}", size_of::<dewey::ontology::Properties>());
    println!("Accessibility   {}", size_of::<dewey::ontology::Accessibility>());
    println!("NodeBounds      {}", size_of::<dewey::ontology::NodeBounds>());
    println!("Cow<str>        {}", size_of::<std::borrow::Cow<'static, str>>());
}
