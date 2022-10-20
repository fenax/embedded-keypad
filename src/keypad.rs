
use core::mem::size_of;
use core::default::Default;
use core::cmp::PartialEq;
use bitmask_enum::bitmask;
use defmt::intern;
use defmt::info;
use defmt::Format;
use embedded_hal::digital::v2::{InputPin, OutputPin};
use paste::paste;

#[macro_export]
macro_rules! count_tt {
    () => { 0 };
    ($odd:tt $($a:tt $b:tt)*) => { (embedded_keypad::count_tt!($($a)*) << 1) | 1 };
    ($($a:tt $even:tt)*) => { embedded_keypad::count_tt!($($a)*) << 1 };
}

#[macro_export]
macro_rules! build_keyboard {
    ($type_name : ident,$data_size:ty,
        [
            $($key_codes:literal)*
        ],
        [
            $($key_names:ident)*
        ],
        [$(
            [$modifier:ident [$($modifier_key:ident),*]]
        ),*]
    ) => {
        #[bitmask_enum::bitmask($data_size)]
        pub enum $type_name {
            $(
                    $key_names = 1<< $key_codes,
            )*
            $(
                $modifier = {
                    let mut val = 0;
                    $(val |= Self::$modifier_key.bits;)*
                    val
                },
            )*
        }
        impl $type_name{
            const KEYMAP_SIZE:usize=embedded_keypad::count_tt!($($key_codes)*);
            const KEYMAP:[usize;$type_name::KEYMAP_SIZE] = [$($key_codes),*];
        }
    };
}



#[macro_export]
macro_rules! build_keymap {
    (   $name:ident,
        $modifiers:ident,
        $textmod:ident,
        $default:literal,
        $format:literal,
        [
            $($($mod:ident)* [$map:literal]),*
        ]
    ) => {
        paste::paste!{
            impl Default for $name{
                fn default() -> Self {
                    Self::none()
                }
            }
            impl $name {
                const fn make_map(keys:&[u8;$name::KEYMAP_SIZE])->[u8;$name::KEYMAP_SIZE]{
                    let mut out = [0u8;$name::KEYMAP_SIZE];
                    let mut i = 0usize;
                    loop{
                        out[$name::KEYMAP[i]] = keys[i];
                        i +=1;
                        if i ==$name::KEYMAP_SIZE{
                            break
                        }
                    }
                    out
                }
                const fn make_layout(keys:&[u8;$name::KEYMAP_SIZE])->[u8;$format.len()]{
                    let mut out = [0u8;$format.len()];
                    let mut i = 0usize;
                    let mut j = 0usize;
                    loop{
                        if $format[i] == b'*'{
                            out[i] = keys[j];
                            j +=1;

                        }else{
                            out[i] = $format[i];
                        }
                        i +=1;
                        if i ==$format.len(){
                            break
                        }
                    }
                    out
                }
                const DEFAULT_MAP : [u8;$name::KEYMAP_SIZE] = $name::make_map($default);
                const DEFAULT_LAYOUT : [u8;$format.len()] = $name::make_layout($default);
                $(
                    const [<$($mod:upper _)* MAP>] : [u8;$name::KEYMAP_SIZE] = $name::make_map($map);
                    const [<$($mod:upper _)* LAYOUT>] : [u8;$format.len()] = $name::make_layout($map);
                )*
            }
            impl embedded_keypad::traits::HasLayout for $name{

            }
            impl embedded_keypad::traits::InnerKeys for $name{
                fn get_text_modifiers(self) -> Self{
                    self.and($name::$textmod)
                }

                fn get_layout(self)  -> &'static[u8]{
                    let mods = self.and($name::$modifiers);

                    $(
                        if $( mods.intersects($name::$mod) &&)* true{
                            &$name::[<$($mod:upper _)* LAYOUT>]
                        }else
                    )*
                    {
                        &$name::DEFAULT_LAYOUT
                    }
                }

                fn get_one_char(self) -> Option<u8> {
                    let no_mod = self.and($name::$modifiers.not()).bits();
                    let mods = self.and($name::$modifiers);
                    if mods.and($name::$textmod.not()).is_none(){
                        let source = $(
                            if $( mods.intersects($name::$mod) &&)* true{
                                $name::[<$($mod:upper _)* MAP>]
                            }else
                        )*
                        {
                            $name::DEFAULT_MAP
                        };
                        if no_mod.count_ones() == 1 {
                            Some(source[no_mod.trailing_zeros() as usize])
                        } else {
                            None
                        }
                    }else{
                        None
                    }
                }
            }
        }

    };
}


pub enum InputState<K>
where 
K: Default+crate::traits::InnerKeys+PartialEq+Copy
 {
    Running(K),
    Updated,
    Validated,
    Overflow,
    NotForMe(K),
}

impl<const T: usize,K> Format for InputBuffer<T,K> 
where 
K: Default+crate::traits::InnerKeys+PartialEq+Copy
{
    fn format(&self, _fmt: defmt::Formatter) {
        let t = intern!("{=[u8]:a}");
        defmt::export::istr(&t);
        let len = self.len();
        defmt::export::usize(&(len + 2));
        for i in 0..self.cursor {
            defmt::export::u8(&self.buffer[i])
        }
        defmt::export::u8(&b'>');
        defmt::export::u8(&b'<');
        for i in self.cursor..len {
            defmt::export::u8(&self.buffer[i])
        }
    }
}

pub struct InputBuffer<const S: usize,K> {
    pub buffer: [u8; S],
///TODO change this shit
    pub left: K,
    pub right: K,
    pub backspace: K,
    pub validate: K,

    last: K,
    ready: bool,
    cursor: usize,
}

impl<const S: usize,K> InputBuffer<S,K>
where 
K: Default+crate::traits::InnerKeys+PartialEq+Copy
{
    pub fn new() -> Self {
        Self {
            buffer: [0u8; S],
            last: K::default(),
            left: K::default(),
            right: K::default(),
            backspace: K::default(),
            validate: K::default(),
            ready: true,
            cursor: 0,
        }
    }
    pub fn len(&self) -> usize {
        let mut len = 0;
        for c in self.buffer {
            if c != 0 {
                len += 1;
            } else {
                break;
            }
        }
        len
    }
    pub fn get_data(&self) -> &[u8] {
        &self.buffer[0..self.len()]
    }
    pub fn get_cursor(&self) -> usize {
        self.cursor
    }
    pub fn clear(&mut self) {
        self.buffer = [0u8; S];
        self.cursor = 0;
    }
    pub fn process_input(&mut self, key: K) -> InputState<K> {
        let mut ret = InputState::NotForMe(key);
        if key != self.last {
            let car = key.get_one_char();

            if let (Some(car), true) = (car, self.ready) {
                self.ready = false;
                if self.cursor >= S || self.buffer[S - 1] != 0 {
                    ret = InputState::Overflow;
                } else {
                    for i in (self.cursor..S - 1).rev() {
                        self.buffer[i + 1] = self.buffer[i];
                    }
                    self.buffer[self.cursor] = car as u8;
                    self.cursor += 1;
                    ret = InputState::Updated;
                }
            } else if key != K::default(){
                if key.get_text_modifiers() == key{
                    // only a text modifier key is pressed
                    ret = InputState::Running(key);
                    self.ready = true;
                }

                if key == self.validate{
                    ret = InputState::Validated;
                }else if key == self.backspace{
                    if self.cursor == 0 {
                        ret = InputState::Overflow;
                    } else {
                        for i in self.cursor..S {
                            self.buffer[i - 1] = self.buffer[i];
                        }
                        self.cursor -= 1;
                        self.buffer[S - 1] = 0;
                        ret = InputState::Updated;
                    }
                }else if key == self.left{
                    if self.cursor == 0 {
                        ret = InputState::Overflow;
                    } else {
                        self.cursor -= 1;
                        ret = InputState::Updated;
                    }
                }else if key == self.right{
                    if self.cursor >= S || self.buffer[self.cursor] == 0 {
                        ret = InputState::Overflow;
                    } else {
                        self.cursor += 1;
                        ret = InputState::Updated;
                    }
                }
            }else{
                ret = InputState::Running(key.get_text_modifiers());
                self.ready = true;

            }
        }
        self.last = key;
        ret
    }
}
