// EMOJI / EMOTICON PICKER DATA
/// A single emoticon entry with a short name and the emoticon string.
pub struct Emoticon {
    pub name: &'static str,
    pub value: &'static str,
}

/// A category of emoticons.
pub struct EmoticonCategory {
    pub name: &'static str,
    pub icon: &'static str,
    pub emoticons: Vec<Emoticon>,
}

/// Build all available emoticon categories.
pub fn categories() -> Vec<EmoticonCategory> {
    vec![
        EmoticonCategory {
            name: "Happy",
            icon: "😊",
            emoticons: vec![
                Emoticon {
                    name: "smile",
                    value: "(•‿•)",
                },
                Emoticon {
                    name: "bright",
                    value: "(◕‿◕)",
                },
                Emoticon {
                    name: "flower",
                    value: "(✿◠‿◠)",
                },
                Emoticon {
                    name: "gentle",
                    value: "(◠‿◠)",
                },
                Emoticon {
                    name: "cool",
                    value: "(⌐■_■)",
                },
                Emoticon {
                    name: "star eyes",
                    value: "(★‿★)",
                },
                Emoticon {
                    name: "happy cat",
                    value: "(^‿^)",
                },
                Emoticon {
                    name: "wink",
                    value: "(◕ᴗ◕✿)",
                },
                Emoticon {
                    name: "joy",
                    value: "(ᵔ◡ᵔ)",
                },
                Emoticon {
                    name: "grin",
                    value: "(｡◕‿◕｡)",
                },
                Emoticon {
                    name: "blush",
                    value: "(⁄ ⁄•⁄ω⁄•⁄ ⁄)",
                },
                Emoticon {
                    name: "proud",
                    value: "(￣ω￣)",
                },
            ],
        },
        EmoticonCategory {
            name: "Sad",
            icon: "😢",
            emoticons: vec![
                Emoticon {
                    name: "crying",
                    value: "(╥_╥)",
                },
                Emoticon {
                    name: "tears",
                    value: "(T_T)",
                },
                Emoticon {
                    name: "weep",
                    value: "(;_;)",
                },
                Emoticon {
                    name: "despair",
                    value: "(ಥ_ಥ)",
                },
                Emoticon {
                    name: "broken",
                    value: "(ᗒᗣᗕ)",
                },
                Emoticon {
                    name: "sigh",
                    value: "(._. )",
                },
                Emoticon {
                    name: "lonely",
                    value: "(´;ω;`)",
                },
                Emoticon {
                    name: "hurt",
                    value: "(っ˘̩╭╮˘̩)っ",
                },
                Emoticon {
                    name: "rain",
                    value: "(ノ_<。)",
                },
                Emoticon {
                    name: "disappointed",
                    value: "(◞‸◟)",
                },
            ],
        },
        EmoticonCategory {
            name: "Angry",
            icon: "😠",
            emoticons: vec![
                Emoticon {
                    name: "table flip",
                    value: "(╯°□°)╯︵ ┻━┻",
                },
                Emoticon {
                    name: "stare",
                    value: "(ಠ_ಠ)",
                },
                Emoticon {
                    name: "side eye",
                    value: "(¬_¬)",
                },
                Emoticon {
                    name: "rage",
                    value: "(ノಠ益ಠ)ノ",
                },
                Emoticon {
                    name: "fury",
                    value: "凸(¬‿¬)",
                },
                Emoticon {
                    name: "grr",
                    value: "(≖_≖)",
                },
                Emoticon {
                    name: "annoyed",
                    value: "(눈_눈)",
                },
                Emoticon {
                    name: "fist",
                    value: "(ง'̀-'́)ง",
                },
                Emoticon {
                    name: "unflip",
                    value: "┬─┬ ノ( ゜-゜ノ)",
                },
                Emoticon {
                    name: "double flip",
                    value: "┻━┻ ︵ ¯\\(ツ)/¯ ︵ ┻━┻",
                },
            ],
        },
        EmoticonCategory {
            name: "Love",
            icon: "❤️",
            emoticons: vec![
                Emoticon {
                    name: "heart eyes",
                    value: "(♥‿♥)",
                },
                Emoticon {
                    name: "love",
                    value: "(◕‿◕✿)",
                },
                Emoticon {
                    name: "cuddle",
                    value: "(´• ω •`)",
                },
                Emoticon {
                    name: "kiss",
                    value: "(◕3◕)",
                },
                Emoticon {
                    name: "hug",
                    value: "(⊃｡•́‿•̀｡)⊃",
                },
                Emoticon {
                    name: "hearts",
                    value: "(♡˙︶˙♡)",
                },
                Emoticon {
                    name: "loving",
                    value: "(｡♥‿♥｡)",
                },
                Emoticon {
                    name: "blushing",
                    value: "(〃▽〃)",
                },
                Emoticon {
                    name: "warm",
                    value: "(◍•ᴗ•◍)❤",
                },
                Emoticon {
                    name: "couple",
                    value: "(♡°▽°♡)",
                },
            ],
        },
        EmoticonCategory {
            name: "Surprise",
            icon: "😲",
            emoticons: vec![
                Emoticon {
                    name: "shocked",
                    value: "(⊙_⊙)",
                },
                Emoticon {
                    name: "gasp",
                    value: "(°o°)",
                },
                Emoticon {
                    name: "wide eyes",
                    value: "(O_O)",
                },
                Emoticon {
                    name: "what",
                    value: "(⊙﹏⊙)",
                },
                Emoticon {
                    name: "omg",
                    value: "(°△°)",
                },
                Emoticon {
                    name: "frozen",
                    value: "Σ(°△°|||)",
                },
                Emoticon {
                    name: "stunned",
                    value: "(゜-゜)",
                },
                Emoticon {
                    name: "woah",
                    value: "(ꏿ﹏ꏿ)",
                },
            ],
        },
        EmoticonCategory {
            name: "Fun",
            icon: "🎉",
            emoticons: vec![
                Emoticon {
                    name: "finger guns",
                    value: "(☞ﾟヮﾟ)☞",
                },
                Emoticon {
                    name: "running",
                    value: "ᕕ(ᐛ)ᕗ",
                },
                Emoticon {
                    name: "sparkles",
                    value: "(ノ◕ヮ◕)ノ*:・゚✧",
                },
                Emoticon {
                    name: "shrug",
                    value: "¯\\_(ツ)_/¯",
                },
                Emoticon {
                    name: "deal with it",
                    value: "(•_•) ( •_•)>⌐■-■ (⌐■_■)",
                },
                Emoticon {
                    name: "party",
                    value: "♪(´ε` )",
                },
                Emoticon {
                    name: "dance",
                    value: "♪♪ \\(^ω^\\)",
                },
                Emoticon {
                    name: "magic",
                    value: "(∩ᄑ_ᄑ)⊃━☆ﾟ.*",
                },
                Emoticon {
                    name: "lenny",
                    value: "( ͡° ͜ʖ ͡°)",
                },
                Emoticon {
                    name: "look",
                    value: "(͡• ͜ʖ ͡•)",
                },
                Emoticon {
                    name: "yolo",
                    value: "~(˘▾˘~)",
                },
                Emoticon {
                    name: "flex",
                    value: "ᕦ(ò_óˇ)ᕤ",
                },
            ],
        },
        EmoticonCategory {
            name: "Animals",
            icon: "🐻",
            emoticons: vec![
                Emoticon {
                    name: "bear",
                    value: "ʕ•ᴥ•ʔ",
                },
                Emoticon {
                    name: "cat",
                    value: "(=^・ω・^=)",
                },
                Emoticon {
                    name: "bunny",
                    value: "(='ω'=)",
                },
                Emoticon {
                    name: "dog",
                    value: "∪・ω・∪",
                },
                Emoticon {
                    name: "fish",
                    value: ">゜)))彡",
                },
                Emoticon {
                    name: "bird",
                    value: "(•ө•)",
                },
                Emoticon {
                    name: "spider",
                    value: "/\\(°_°)/\\",
                },
                Emoticon {
                    name: "penguin",
                    value: "(ᵔᴥᵔ)",
                },
                Emoticon {
                    name: "koala",
                    value: "ʕ·ᴥ·ʔ",
                },
                Emoticon {
                    name: "mouse",
                    value: "~~(,,꒪꒳꒪,,)~~",
                },
            ],
        },
        EmoticonCategory {
            name: "Misc",
            icon: "✨",
            emoticons: vec![
                Emoticon {
                    name: "thumbs up",
                    value: "(b╰▿╯)b",
                },
                Emoticon {
                    name: "peace",
                    value: "✌(◕‿-)✌",
                },
                Emoticon {
                    name: "salute",
                    value: "(￣^￣)ゞ",
                },
                Emoticon {
                    name: "sleeping",
                    value: "(¦3[▓▓]",
                },
                Emoticon {
                    name: "thinking",
                    value: "(ᓀ ᓀ)",
                },
                Emoticon {
                    name: "confused",
                    value: "(⊙_☉)",
                },
                Emoticon {
                    name: "dizzy",
                    value: "(@_@)",
                },
                Emoticon {
                    name: "nervous",
                    value: "(°ロ°) !",
                },
                Emoticon {
                    name: "whatever",
                    value: "┐(´ー｀)┌",
                },
                Emoticon {
                    name: "bye",
                    value: "(ノ´ з `)ノ",
                },
                Emoticon {
                    name: "facepalm",
                    value: "(－‸ლ)",
                },
                Emoticon {
                    name: "music",
                    value: "(˳˘ ɜ˘)˳ ♬♪♫",
                },
            ],
        },
    ]
}

/// Get the number of emoticons in a specific category from a categories list.
pub fn category_item_count(cats: &[EmoticonCategory], category_index: usize) -> usize {
    cats.get(category_index)
        .map(|c| c.emoticons.len())
        .unwrap_or(0)
}

/// Get an emoticon value by category and item index from a categories list.
pub fn get_emoticon(
    cats: &[EmoticonCategory],
    category_index: usize,
    item_index: usize,
) -> Option<&str> {
    cats.get(category_index)
        .and_then(|c| c.emoticons.get(item_index))
        .map(|e| e.value)
}

/// Search result: category index, item index, emoticon name, emoticon value.
#[allow(dead_code)]
pub struct SearchResult {
    pub cat_idx: usize,
    pub item_idx: usize,
    pub name: &'static str,
    pub value: &'static str,
}

/// Get filtered emoticons matching a search query.
pub fn search_emoticons(cats: &[EmoticonCategory], query: &str) -> Vec<SearchResult> {
    let query_lower = query.to_lowercase();
    let mut results = Vec::new();

    for (cat_idx, category) in cats.iter().enumerate() {
        for (item_idx, emoticon) in category.emoticons.iter().enumerate() {
            if emoticon.name.to_lowercase().contains(&query_lower)
                || category.name.to_lowercase().contains(&query_lower)
                || emoticon.value.contains(query)
            {
                results.push(SearchResult {
                    cat_idx,
                    item_idx,
                    name: emoticon.name,
                    value: emoticon.value,
                });
            }
        }
    }

    results
}
