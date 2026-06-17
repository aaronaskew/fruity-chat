const FRUIT: [&str; 20] = [
    "🍇",
    "🍈",
    "🍉",
    "🍊",
    "🍋",
    "🍋‍🟩",
    "🍌",
    "🍍",
    "🥭",
    "🍎",
    "🍏",
    "🍐",
    "🍑",
    "🍒",
    "🍓",
    "🫐",
    "🥝",
    "🍅",
    "🫒",
    "🥥",
];

pub fn random_fruit(len: usize) -> String {
    let mut s = String::new();
    for _ in 0..len {
        s.push_str(FRUIT[rand::random_range(..FRUIT.len())]);
    }

    s
}
