use cosmic::{
    Element,
    iced::{Border, widget::stack},
    widget::{container, space, svg, text},
};
use std::{collections::BTreeMap, sync::LazyLock};

// Embedded artwork has no runtime asset-path or network dependency. Prepare
// each country's SVG handle once, only when it is needed.
macro_rules! flag_assets {
    ($($code:literal),* $(,)?) => {
        static FLAGS: LazyLock<BTreeMap<&'static str, LazyLock<svg::Handle>>> =
            LazyLock::new(|| BTreeMap::from([
                $(($code, LazyLock::<svg::Handle>::new(|| rounded_flag(include_str!(
                    concat!("../data/flags/4x3/", $code, ".svg")
                ))))),*
            ]));
    };
}

flag_assets!(
    "ad", "ae", "af", "ag", "ai", "al", "am", "ao", "aq", "ar", "as", "at", "au", "aw", "ax", "az",
    "ba", "bb", "bd", "be", "bf", "bg", "bh", "bi", "bj", "bl", "bm", "bn", "bo", "bq", "br", "bs",
    "bt", "bv", "bw", "by", "bz", "ca", "cc", "cd", "cf", "cg", "ch", "ci", "ck", "cl", "cm", "cn",
    "co", "cp", "cr", "cu", "cv", "cw", "cx", "cy", "cz", "de", "dg", "dj", "dk", "dm", "do", "dz",
    "ec", "ee", "eg", "eh", "er", "es", "et", "eu", "fi", "fj", "fk", "fm", "fo", "fr", "ga", "gb",
    "gd", "ge", "gf", "gg", "gh", "gi", "gl", "gm", "gn", "gp", "gq", "gr", "gs", "gt", "gu", "gw",
    "gy", "hk", "hm", "hn", "hr", "ht", "hu", "ic", "id", "ie", "il", "im", "in", "io", "iq", "ir",
    "is", "it", "je", "jm", "jo", "jp", "ke", "kg", "kh", "ki", "km", "kn", "kp", "kr", "kw", "ky",
    "kz", "la", "lb", "lc", "li", "lk", "lr", "ls", "lt", "lu", "lv", "ly", "ma", "mc", "md", "me",
    "mf", "mg", "mh", "mk", "ml", "mm", "mn", "mo", "mp", "mq", "mr", "ms", "mt", "mu", "mv", "mw",
    "mx", "my", "mz", "na", "nc", "ne", "nf", "ng", "ni", "nl", "no", "np", "nr", "nu", "nz", "om",
    "pa", "pc", "pe", "pf", "pg", "ph", "pk", "pl", "pm", "pn", "pr", "ps", "pt", "pw", "py", "qa",
    "re", "ro", "rs", "ru", "rw", "sa", "sb", "sc", "sd", "se", "sg", "sh", "si", "sj", "sk", "sl",
    "sm", "sn", "so", "sr", "ss", "st", "sv", "sx", "sy", "sz", "tc", "td", "tf", "tg", "th", "tj",
    "tk", "tl", "tm", "tn", "to", "tr", "tt", "tv", "tw", "tz", "ua", "ug", "um", "un", "us", "uy",
    "uz", "va", "vc", "ve", "vg", "vi", "vn", "vu", "wf", "ws", "xk", "ye", "yt", "za", "zm", "zw",
);

fn rounded_flag(source: &str) -> svg::Handle {
    svg::Handle::from_memory(
        format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="640" height="480" viewBox="0 0 640 480">
<defs><clipPath id="finance-dashboard-rounded"><rect width="640" height="480" rx="24"/></clipPath></defs>
<g clip-path="url(#finance-dashboard-rounded)">{source}</g></svg>"##
        )
        .into_bytes(),
    )
}

fn handle(country: &str) -> Option<svg::Handle> {
    FLAGS
        .get(country.trim().to_ascii_lowercase().as_str())
        .map(|flag| LazyLock::force(flag).clone())
}

pub fn view<Message: 'static>(country: &str, width: f32) -> Element<'static, Message> {
    let height = width * 0.75;
    let graphic: Element<'static, Message> = match handle(country) {
        Some(handle) => svg(handle).width(width).height(height).into(),
        None => container(text("—").size(height * 0.5))
            .center_x(width)
            .center_y(height)
            .into(),
    };
    let outline =
        container(space())
            .width(width)
            .height(height)
            .class(cosmic::theme::Container::custom(move |theme| {
                container::Style {
                    border: Border {
                        color: theme.cosmic().background(theme.transparent).divider.into(),
                        width: (width / 32.0).min(1.0),
                        radius: (width * 24.0 / 640.0).into(),
                    },
                    ..Default::default()
                }
            }));
    stack![graphic, outline].width(width).height(height).into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmic_applet_finance_dashboard::{config::Settings, market};

    #[test]
    fn all_bundled_flags_and_default_currencies_have_registry_entries() {
        let assets =
            std::fs::read_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/data/flags/4x3")).unwrap();
        let mut count = 0;
        for asset in assets {
            let path = asset.unwrap().path();
            if path.extension().is_some_and(|extension| extension == "svg") {
                let code = path.file_stem().unwrap().to_str().unwrap();
                assert!(FLAGS.contains_key(code), "Unregistered flag: {code}");
                count += 1;
            }
        }
        assert_eq!(count, FLAGS.len());
        for pair in Settings::default().forex {
            for currency in [&pair[..3], &pair[3..]] {
                let (_, country) = market::currency(currency);
                assert!(handle(country).is_some(), "Missing flag for {currency}");
            }
        }
    }

    #[test]
    fn country_lookup_normalizes_codes_and_unknowns_use_the_neutral_fallback() {
        for code in ["US", "CN", "SG", "EU", "JP", "HK", "GB", "XK"] {
            let expected = handle(code).unwrap();
            assert_eq!(
                handle(&format!(" {} ", code.to_ascii_lowercase())),
                Some(expected)
            );
        }
        for unknown in ["", " ", "??", "ZZ", "xx", "USA", "🇸🇬", "../us"] {
            assert!(handle(unknown).is_none(), "Unexpected flag for {unknown}");
        }
    }
}
