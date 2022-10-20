
pub trait HasLayout{
}
pub trait InnerKeys{
    fn get_text_modifiers(self) -> Self;
    fn get_one_char(self) -> Option<u8>;
    fn get_layout(self)  -> &'static[u8];

}