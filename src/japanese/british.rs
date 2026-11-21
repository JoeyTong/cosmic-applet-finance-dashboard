// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! British spellings for dictionary English. Whole words only, so a stem is
//! not rewritten inside a different word. Applying it twice leaves the text unchanged.

pub fn to_british(text: &str) -> String {
    let mut output = String::with_capacity(text.len());
    let mut word = String::new();
    for character in text.chars() {
        if character.is_ascii_alphabetic() {
            word.push(character);
        } else {
            if !word.is_empty() {
                output.push_str(&convert_word(&word));
                word.clear();
            }
            output.push(character);
        }
    }
    if !word.is_empty() {
        output.push_str(&convert_word(&word));
    }
    output
}

fn convert_word(word: &str) -> String {
    let lower = word.to_ascii_lowercase();
    let Some(british) = spelling(lower.as_str()) else {
        return word.to_owned();
    };
    match case_of(word) {
        Case::Upper => british.to_ascii_uppercase(),
        Case::Capital => {
            let mut chars = british.chars();
            match chars.next() {
                Some(first) => format!("{}{}", first.to_ascii_uppercase(), chars.as_str()),
                None => String::new(),
            }
        }
        Case::Lower => british.to_owned(),
    }
}

enum Case {
    Lower,
    Capital,
    Upper,
}

fn case_of(word: &str) -> Case {
    let mut letters = word
        .chars()
        .filter(|character| character.is_ascii_alphabetic());
    let Some(first) = letters.next() else {
        return Case::Lower;
    };
    if first.is_ascii_lowercase() {
        Case::Lower
    } else if letters.all(|character| character.is_ascii_uppercase()) {
        Case::Upper
    } else {
        Case::Capital
    }
}

fn spelling(word: &str) -> Option<&'static str> {
    Some(match word {
        "color" | "colour" => "colour",
        "colors" | "colours" => "colours",
        "colored" | "coloured" => "coloured",
        "coloring" | "colouring" => "colouring",
        "coloration" | "colouration" => "colouration",
        "favor" | "favour" => "favour",
        "favors" | "favours" => "favours",
        "favorite" | "favourite" => "favourite",
        "favorites" | "favourites" => "favourites",
        "favorable" | "favourable" => "favourable",
        "honor" | "honour" => "honour",
        "honors" | "honours" => "honours",
        "honored" | "honoured" => "honoured",
        "honorable" | "honourable" => "honourable",
        "labor" | "labour" => "labour",
        "labors" | "labours" => "labours",
        "neighbor" | "neighbour" => "neighbour",
        "neighbors" | "neighbours" => "neighbours",
        "neighborhood" | "neighbourhood" => "neighbourhood",
        "behavior" | "behaviour" => "behaviour",
        "behaviors" | "behaviours" => "behaviours",
        "humor" | "humour" => "humour",
        "rumor" | "rumour" => "rumour",
        "rumors" | "rumours" => "rumours",
        "vapor" | "vapour" => "vapour",
        "harbor" | "harbour" => "harbour",
        "harbors" | "harbours" => "harbours",
        "flavor" | "flavour" => "flavour",
        "flavors" | "flavours" => "flavours",
        "endeavor" | "endeavour" => "endeavour",
        "endeavors" | "endeavours" => "endeavours",
        "savior" | "saviour" => "saviour",
        "armor" | "armour" => "armour",
        "splendor" | "splendour" => "splendour",
        "vigor" | "vigour" => "vigour",
        "odor" | "odour" => "odour",
        "odors" | "odours" => "odours",
        "mold" | "mould" => "mould",
        "molds" | "moulds" => "moulds",
        "mustache" | "moustache" => "moustache",
        "gray" | "grey" => "grey",
        "airplane" | "aeroplane" => "aeroplane",
        "airplanes" | "aeroplanes" => "aeroplanes",
        "aluminum" | "aluminium" => "aluminium",
        "artifact" | "artefact" => "artefact",
        "artifacts" | "artefacts" => "artefacts",
        "aging" | "ageing" => "ageing",
        "jewelry" | "jewellery" => "jewellery",
        "catalog" | "catalogue" => "catalogue",
        "catalogs" | "catalogues" => "catalogues",
        "dialog" | "dialogue" => "dialogue",
        "dialogs" | "dialogues" => "dialogues",
        "defense" | "defence" => "defence",
        "defenses" | "defences" => "defences",
        "offense" | "offence" => "offence",
        "offenses" | "offences" => "offences",
        "license" | "licence" => "licence",
        "licenses" | "licences" => "licences",
        "meter" | "metre" => "metre",
        "meters" | "metres" => "metres",
        "kilometer" | "kilometre" => "kilometre",
        "kilometers" | "kilometres" => "kilometres",
        "centimeter" | "centimetre" => "centimetre",
        "centimeters" | "centimetres" => "centimetres",
        "millimeter" | "millimetre" => "millimetre",
        "millimeters" | "millimetres" => "millimetres",
        "liter" | "litre" => "litre",
        "liters" | "litres" => "litres",
        "theater" | "theatre" => "theatre",
        "theaters" | "theatres" => "theatres",
        "center" | "centre" => "centre",
        "centers" | "centres" => "centres",
        "fiber" | "fibre" => "fibre",
        "fibers" | "fibres" => "fibres",
        "caliber" | "calibre" => "calibre",
        "saber" | "sabre" => "sabre",
        "somber" | "sombre" => "sombre",
        "meager" | "meagre" => "meagre",
        "analyze" | "analyse" => "analyse",
        "analyzes" | "analyses" => "analyses",
        "analyzed" | "analysed" => "analysed",
        "paralyze" | "paralyse" => "paralyse",
        "paralyzed" | "paralysed" => "paralysed",
        "organize" | "organise" => "organise",
        "organizes" | "organises" => "organises",
        "organized" | "organised" => "organised",
        "organizing" | "organising" => "organising",
        "organization" | "organisation" => "organisation",
        "organizations" | "organisations" => "organisations",
        "recognize" | "recognise" => "recognise",
        "recognized" | "recognised" => "recognised",
        "realize" | "realise" => "realise",
        "realized" | "realised" => "realised",
        "apologize" | "apologise" => "apologise",
        "apologized" | "apologised" => "apologised",
        "authorize" | "authorise" => "authorise",
        "authorized" | "authorised" => "authorised",
        "characterize" | "characterise" => "characterise",
        "emphasize" | "emphasise" => "emphasise",
        "memorize" | "memorise" => "memorise",
        "minimize" | "minimise" => "minimise",
        "specialize" | "specialise" => "specialise",
        "summarize" | "summarise" => "summarise",
        "symbolize" | "symbolise" => "symbolise",
        "utilize" | "utilise" => "utilise",
        "civilize" | "civilise" => "civilise",
        "criticize" | "criticise" => "criticise",
        "traveled" | "travelled" => "travelled",
        "traveling" | "travelling" => "travelling",
        "traveler" | "traveller" => "traveller",
        "travelers" | "travellers" => "travellers",
        "canceled" | "cancelled" => "cancelled",
        "canceling" | "cancelling" => "cancelling",
        "modeled" | "modelled" => "modelled",
        "modeling" | "modelling" => "modelling",
        "labeled" | "labelled" => "labelled",
        "labeling" | "labelling" => "labelling",
        "fulfill" | "fulfil" => "fulfil",
        "fulfillment" | "fulfilment" => "fulfilment",
        "enrollment" | "enrolment" => "enrolment",
        "skillful" | "skilful" => "skilful",
        "skeptic" | "sceptic" => "sceptic",
        "skeptical" | "sceptical" => "sceptical",
        "plow" | "plough" => "plough",
        "plows" | "ploughs" => "ploughs",
        "tire" | "tyre" => "tyre",
        "tires" | "tyres" => "tyres",
        "check" => return None,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::to_british;

    #[test]
    fn american_spellings_become_british_without_touching_other_words() {
        assert_eq!(
            to_british("favorite color theater"),
            "favourite colour theatre"
        );
        assert_eq!(to_british("Color"), "Colour");
        assert_eq!(to_british("GRAY"), "GREY");
        assert_eq!(to_british("a cat and a colorize"), "a cat and a colorize");
        assert_eq!(to_british("colour"), "colour");
        assert_eq!(
            to_british("to organize the neighborhood"),
            "to organise the neighbourhood"
        );
    }
}
