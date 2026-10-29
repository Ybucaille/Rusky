//! Prononciation « à la française » calculée depuis le russe accentué.
//!
//! L'accent tonique s'écrit avec l'accent aigu combinant (молоко́) ; ё est
//! toujours accentué. La syllabe accentuée sort en MAJUSCULES : ma-la-KO.
//! Règles appliquées : о sans accent → « a », е/я sans accent → « i »,
//! consonnes finales assourdies, -ого/-его → « -ava/-iva », -тся/-ться → « -tsa ».

const ACUTE: char = '\u{301}';
const VOWELS: &str = "аеёиоуыэюяАЕЁИОУЫЭЮЯ";

pub fn pron(text: &str) -> String {
    // Découpe en mots et séparateurs, pour que в, с, к prennent le son du mot suivant.
    let mut tokens: Vec<(String, String)> = vec![(String::new(), String::new())];
    for c in text.chars() {
        let last = tokens.last_mut().unwrap();
        if (c.is_alphabetic() || c == ACUTE || c == '-' && !last.0.is_empty()) && last.1.is_empty() {
            last.0.push(c);
        } else if c.is_alphabetic() || c == ACUTE {
            tokens.push((c.to_string(), String::new()));
        } else {
            last.1.push(c);
        }
    }
    let mut out = String::new();
    for (i, (word, sep)) in tokens.iter().enumerate() {
        let next = tokens.get(i + 1).and_then(|t| t.0.to_lowercase().chars().next());
        out.push_str(&match (word.to_lowercase().as_str(), sep.as_str() == " ", next) {
            // в парк → « f park », с дру́гом → « z drou… », к го́роду → « g ga… »
            ("в" | "с" | "к", true, Some(n)) if "пфктшсхцчщ".contains(n) => {
                if word.to_lowercase() == "в" { "f".into() } else { pron_word(word) }
            }
            ("с", true, Some(n)) if "бгджз".contains(n) => "z".into(),
            ("к", true, Some(n)) if "бгджз".contains(n) => "g".into(),
            _ => pron_word(word),
        });
        out.push_str(sep);
    }
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
        // по-ру́сски, ну́-ка : chaque morceau à part ; un morceau d'une syllabe porteur
        // de l'accent passe en majuscules, puisque le mot entier en a plusieurs.
        let pieces: Vec<&str> = word.split('-').collect();
        let syllables = word.chars().filter(|c| is_vowel(*c)).count();
        return pieces
            .iter()
            .map(|p| {
                let out = pron_word(p);
                let own = p.chars().filter(|c| is_vowel(*c)).count();
                if syllables > 1 && own == 1 && (p.contains(ACUTE) || p.contains(['ё', 'Ё'])) { out.to_uppercase() } else { out }
            })
            .collect::<Vec<_>>()
            .join("-");
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
    // жч, сч, зч se disent comme щ : мужчи́на, счастли́вый.
    for i in (0..letters.len().saturating_sub(1)).rev() {
        if "жсз".contains(letters[i].0) && letters[i + 1].0 == 'ч' {
            letters[i + 1].0 = 'щ';
            letters.remove(i);
        }
    }
    // Consonnes muettes : со́лнце, пра́здник, чу́вствовать, изве́стный, се́рдце, счастли́вый.
    for (pattern, silent) in [("лнц", 0), ("здн", 1), ("стн", 1), ("вств", 0), ("рдц", 1), ("стл", 1)] {
        let pat: Vec<char> = pattern.chars().collect();
        while let Some(i) = letters.windows(pat.len()).position(|w| w.iter().map(|l| l.0).eq(pat.iter().copied())) {
            letters.remove(i + silent);
        }
    }
    let n_vowels = letters.iter().filter(|(c, _)| is_vowel(*c)).count();
    // Pas d'accent indiqué sur un mot d'une syllabe : il est accentué.
    // Petits mots outils sans accent propre : по, до, не… (« pa », « da », « ni »).
    let clitic = n_vowels == 1
        && !letters.iter().any(|l| l.1)
        && ["по", "до", "на", "за", "не", "ни", "от", "об", "под", "над", "из", "без", "о"].contains(&plain.as_str());
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
            // Plus de voyelle ensuite : la consonne s'assourdit (по́езд → PO-yist).
            let last = !flat[k + 1..].iter().any(|x| is_vowel(x.0));
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
    let mut res = joined.join("-").replace("stch", "chtch").replace("STCH", "CHTCH").replace("jtch", "chtch").replace("JTCH", "CHTCH");
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
    // Une sourde devient sonore devant б, г, д, ж, з (отдыха́ть → ad-dy-KHAT, сде́лать → ZDIÉ-lat).
    let voiced_next = next.is_some_and(|n| "бгджз".contains(n));
    // Une consonne sonore devient sourde en fin de mot ou devant une sourde (за́втра → ZAF-tra).
    let word_end = word_end || next.is_some_and(|n| "пфктшсхцчщ".contains(n));
    let front_vowel_next = next.is_some_and(|n| "еёиыэюя".contains(n));
    let s = match c {
        'а' => "a",
        'б' => if word_end { "p" } else { "b" },
        'в' => if word_end { "f" } else { "v" },
        'г' if next.is_some_and(|n| n == 'к' || n == 'ч') => "kh",   // лёгкий, облегче́ние
        'г' => if word_end { "k" } else if front_vowel_next { "gu" } else { "g" },
        'д' => if word_end { "t" } else { "d" },
        'е' if prev.is_some_and(|p| "жшц".contains(p)) => if stressed || very_last { "é" } else { "i" },
        'е' => match (after_cons, stressed) {
            (true, true) => "ié",
            (true, false) => if very_last { "ié" } else { "i" },
            (false, true) => "yé",
            (false, false) => if very_last { "yé" } else { "yi" },
        },
        // После ж, ч, ш, щ : « o » (жёлтый, ещё), ailleurs « io » / « yo ».
        'ё' if prev.is_some_and(|p| "жчшщ".contains(p)) => "o",
        'ё' => if after_cons { "io" } else { "yo" },
        'ж' => if word_end { "ch" } else { "j" },
        'з' => if word_end { "s" } else { "z" },
        'и' => "i",
        'й' => "ï",
        'к' => if voiced_next { "g" } else { "k" },
        'л' => "l",
        'м' => "m",
        'н' => "n",
        'о' => if stressed { "o" } else { "a" },
        'п' => if voiced_next { "b" } else { "p" },
        'р' => "r",
        'с' => if voiced_next { "z" } else { "s" }, // les tirets séparent les syllabes : pas de « z » à la française
        'т' => if voiced_next { "d" } else { "t" },
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
            ("по́езд", "PO-yist"),
            ("со́лнце", "SON-tsé"),
            ("пра́здник", "PRAZ-nik"),
            ("изве́стный", "iz-VIES-nyï"),
            ("в парк", "f park"),
            ("жёлтый", "JOL-tyï"),
            ("ну́-ка", "NOU-ka"),
            ("сыр-бо́р", "syr-BOR"),
            ("ещё", "yi-CHTCHO"),
            ("мужчи́на", "mou-CHTCHI-na"),
            ("отдыха́ть", "ad-dy-KHAT"),
            ("лёгкий", "LIOKH-kiï"),
            ("гру́стный", "GROUS-nyï"),
            ("с дру́гом", "z DROU-gam"),
            ("жена́", "ji-NA"),
            ("же", "jé"),
            ("счита́ть", "chtchi-TAT"),
        ] {
            assert_eq!(pron(ru), fr, "{ru}");
        }
    }
}


