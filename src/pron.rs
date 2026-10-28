//! Prononciation « à la française » calculée depuis le russe accentué.
//!
//! L'accent tonique s'écrit avec l'accent aigu combinant (молоко́) ; ё est
//! toujours accentué. La syllabe accentuée sort en MAJUSCULES : ma-la-KO.
//! Règles appliquées : о sans accent → « a », е/я sans accent → « i »,
//! consonnes finales assourdies, -ого/-его → « -ava/-iva », -тся/-ться → « -tsa ».

const ACUTE: char = '\u{301}';
const VOWELS: &str = "аеёиоуыэюяАЕЁИОУЫЭЮЯ";

pub fn pron(text: &str) -> String {
    let mut out = String::new();
    let mut word = String::new();
    for c in text.chars() {
        if c.is_alphabetic() || c == ACUTE || c == '-' && !word.is_empty() {
            word.push(c);
        } else {
            out.push_str(&pron_word(&word));
            word.clear();
            out.push(c);
        }
    }
    out.push_str(&pron_word(&word));
    out
}

struct Syl {
    text: String,
    stressed: bool,
}

fn pron_word(word: &str) -> String {
    if word.is_empty() {
        return String::new();
    }
    if word.contains('-') {
        // по-ру́сски : chaque morceau à part.
        return word.split('-').map(pron_word).collect::<Vec<_>>().join("-");
    }
    let lower: Vec<char> = word.to_lowercase().chars().collect();
    let plain: String = lower.iter().filter(|&&c| c != ACUTE).collect();
    if !plain.chars().any(|c| c.is_alphabetic() && is_cyrillic(c)) {
        return word.to_string();
    }
    match plain.as_str() {
        "что" => return "chto".into(),
        "чтобы" => return "CHTO-by".into(),
        "конечно" => return "ka-NIECH-na".into(),
        "скучно" => return "SKOUCH-na".into(),
        "сегодня" => return "si-VOD-nia".into(),
        "пожалуйста" => return "pa-JA-lous-ta".into(),
        "в" => return "v".into(),
        "с" => return "s".into(),
        "к" => return "k".into(),
        "сегодняшний" => return "si-VOD-nich-niï".into(),
        _ => {}
    }

    // -тся / -ться en fin de verbe : « tsa ».
    for suffix in ["ться", "тся"] {
        if plain.len() > suffix.len() + 2 && plain.ends_with(suffix) {
            let n = word.chars().count() - suffix.chars().count();
            let stem: String = word.chars().take(n).collect();
            return format!("{}-tsa", pron_word(&stem));
        }
    }

    // Lettres et accent : (lettre, accentuée ?)
    let mut letters: Vec<(char, bool)> = Vec::new();
    for &c in &lower {
        if c == ACUTE {
            if let Some(l) = letters.last_mut() {
                l.1 = true;
            }
        } else {
            letters.push((c, c == 'ё'));
        }
    }
    let n_vowels = letters.iter().filter(|(c, _)| is_vowel(*c)).count();
    // Pas d'accent indiqué sur un mot d'une syllabe : il est accentué.
    // Petits mots outils sans accent propre : по, до, не… (« pa », « da », « ni »).
    let clitic = n_vowels == 1
        && !letters.iter().any(|l| l.1)
        && ["по", "до", "на", "за", "не", "ни", "но", "от", "об", "под", "над", "из", "без", "о"].contains(&plain.as_str());
    if clitic {
        return letters
            .iter()
            .map(|&(c, _)| match c {
                'о' => "a".to_string(),
                'е' => "i".to_string(),
                _ => letter(c, None, None, false, true, false),
            })
            .collect();
    }
    if n_vowels == 1 {
        for l in letters.iter_mut() {
            if is_vowel(l.0) {
                l.1 = true;
            }
        }
    }
    let has_stress = letters.iter().any(|l| l.1);

    // -ого / -его en fin de mot : г se dit « v ».
    let len = letters.len();
    if len >= 3 && letters[len - 2].0 == 'г' && letters[len - 1].0 == 'о' && matches!(letters[len - 3].0, 'о' | 'е') && !["много", "строго", "дорого", "немного"].iter().any(|w| plain.ends_with(w)) {
        letters[len - 2].0 = 'в';
    }

    // Découpe en syllabes : consonnes + voyelle ; dans un groupe de consonnes,
    // la première reste avec la syllabe d'avant (ZAF-tra, KOCH-ka).
    let mut groups: Vec<Vec<(char, bool)>> = vec![vec![]];
    for (i, &l) in letters.iter().enumerate() {
        groups.last_mut().unwrap().push(l);
        if is_vowel(l.0) {
            let rest = &letters[i + 1..];
            if rest.iter().any(|x| is_vowel(x.0)) {
                let cl: Vec<char> = rest.iter().take_while(|x| !is_vowel(x.0)).map(|x| x.0).collect();
                let cons = cl.iter().filter(|&&x| !is_sign(x)).count();
                // Consonne + л/р reste groupé en début de syllabe : liou-BLIOU.
                let glide = cons == 2 && "лр".contains(cl[cl.len() - 1]) && !"лрмнй".contains(cl[0]) || cons == 2 && cl[0] == 'м' && cl[1] == 'н';
                if cons >= 2 && !glide {
                    // la première consonne (et son signe éventuel) reste ici
                    let mut j = i + 1;
                    groups.last_mut().unwrap().push(letters[j]);
                    j += 1;
                    while j < letters.len() && is_sign(letters[j].0) {
                        groups.last_mut().unwrap().push(letters[j]);
                        j += 1;
                    }
                    groups.push(vec![]);
                    // les lettres déjà poussées seront sautées
                    for _ in i + 1..j {
                        groups.last_mut().unwrap().push(('\0', false));
                    }
                } else {
                    groups.push(vec![]);
                }
            }
        }
    }
    // Retire les marqueurs de lettres déjà placées.
    let mut seq: Vec<Vec<(char, bool)>> = Vec::new();
    let mut skip = 0;
    for g in groups {
        let mut v = Vec::new();
        for l in g {
            if l.0 == '\0' {
                skip += 1;
            } else if skip > 0 {
                skip -= 1;
            } else {
                v.push(l);
            }
        }
        if !v.is_empty() {
            seq.push(v);
        }
    }

    // Transcription lettre à lettre, avec le contexte du mot entier.
    let flat: Vec<(char, bool)> = seq.iter().flatten().copied().collect();
    let mut syls: Vec<Syl> = Vec::new();
    let mut k = 0;
    for g in &seq {
        let mut s = String::new();
        let mut stressed = false;
        for _ in g {
            let (c, acc) = flat[k];
            stressed |= acc;
            let prev = if k > 0 { Some(flat[k - 1].0) } else { None };
            let next = flat.get(k + 1).map(|x| x.0);
            let last = k + 1 == flat.len() || flat[k + 1..].iter().all(|x| is_sign(x.0));
            let stress = acc || !has_stress;
            // Syllabe fermée (consonne puis consonne ou fin) : « ie » plutôt que « ié ».
            let closed = next.is_some_and(|n| !is_vowel(n))
                && flat.get(k + 2).is_none_or(|x| !is_vowel(x.0));
            let mut t = letter(c, prev, next, last, stress, k == flat.len() - 1);
            if closed {
                t = t.replace('é', "e");
            }
            s.push_str(&t);
            k += 1;
        }
        syls.push(Syl { text: s, stressed });
    }

    let joined: Vec<String> = syls
        .iter()
        .map(|s| if s.stressed && n_vowels > 1 { s.text.to_uppercase() } else { s.text.clone() })
        .collect();
    let mut res = joined.join("-");
    // Consonne doublée de part et d'autre d'un tiret : it-TI → i-TI, ROUS-ski → ROUS-ki.
    for c in ["t", "s", "k", "n", "m", "l", "r", "p", "f"] {
        let (l, u) = (c.to_string(), c.to_uppercase());
        for (a, b) in [(&l, &l), (&l, &u), (&u, &l), (&u, &u)] {
            res = res.replace(&format!("{a}-{b}"), &format!("-{b}"));
        }
    }
    res.replace("--", "-").trim_matches('-').to_string()
}

fn letter(c: char, prev: Option<char>, next: Option<char>, word_end: bool, stressed: bool, very_last: bool) -> String {
    let after_cons = prev.is_some_and(|p| !is_vowel(p) && !is_sign(p));
    // Une consonne sonore devient sourde en fin de mot ou devant une sourde (за́втра → ZAF-tra).
    let word_end = word_end || next.is_some_and(|n| "пфктшсхцчщ".contains(n));
    let front_vowel_next = next.is_some_and(|n| "еёиыэюя".contains(n));
    let s = match c {
        'а' => "a",
        'б' => if word_end { "p" } else { "b" },
        'в' => if word_end { "f" } else { "v" },
        'г' => if word_end { "k" } else if front_vowel_next { "gu" } else { "g" },
        'д' => if word_end { "t" } else { "d" },
        'е' => match (after_cons, stressed) {
            (true, true) => "ié",
            (true, false) => if very_last { "ié" } else { "i" },
            (false, true) => "yé",
            (false, false) => if very_last { "yé" } else { "yi" },
        },
        'ё' => if after_cons { "io" } else { "yo" },
        'ж' => if word_end { "ch" } else { "j" },
        'з' => if word_end { "s" } else { "z" },
        'и' => "i",
        'й' => "ï",
        'к' => "k",
        'л' => "l",
        'м' => "m",
        'н' => "n",
        'о' => if stressed { "o" } else { "a" },
        'п' => "p",
        'р' => "r",
        'с' => "s", // les tirets séparent les syllabes : pas de « z » à la française
        'т' => "t",
        'у' => "ou",
        'ф' => "f",
        'х' => "kh",
        'ц' => "ts",
        'ч' => "tch",
        'ш' => "ch",
        'щ' => "chtch",
        'ъ' | 'ь' => "",
        'ы' => "y",
        'э' => "è",
        'ю' => if after_cons { "iou" } else { "you" },
        'я' => match (after_cons, stressed || very_last) {
            (true, true) => "ia",
            (true, false) => "i",
            (false, true) => "ya",
            (false, false) => "yi",
        },
        other => return other.to_string(),
    };
    s.to_string()
}

fn is_vowel(c: char) -> bool {
    VOWELS.contains(c)
}

fn is_sign(c: char) -> bool {
    c == 'ь' || c == 'ъ'
}

fn is_cyrillic(c: char) -> bool {
    ('\u{400}'..='\u{4FF}').contains(&c)
}

#[cfg(test)]
mod tests {
    use super::pron;

    #[test]
    fn common_words() {
        for (ru, fr) in [
            ("молоко́", "ma-la-KO"),
            ("хорошо́", "kha-ra-CHO"),
            ("за́втра", "ZAF-tra"),
            ("хлеб", "khliep"),
            ("сего́дня", "si-VOD-nia"),
            ("спаси́бо", "spa-SI-ba"),
            ("кни́ги", "KNI-gui"),
            ("ко́шка", "KOCH-ka"),
            ("го́род", "GO-rat"),
            ("большо́го", "bal-CHO-va"),
            ("у́лица", "OU-li-tsa"),
            ("семья́", "si-MYA"),
            ("что", "chto"),
            ("Я люблю́ ма́му.", "ya liou-BLIOU MA-mou."),
            ("учи́ться", "ou-TCHI-tsa"),
            ("по-ру́сски", "pa-ROU-ski"),
            ("идти́", "i-TI"),
        ] {
            assert_eq!(pron(ru), fr, "{ru}");
        }
    }
}

