//! Works out who a transaction was with from its statement description, entirely offline.
//!
//! Card statements print a merchant descriptor such as "UNIQLO Europe LONDON GB":
//! the merchant, the city and a two-letter country code. Well-known merchants are recognised
//! by keyword and given a clean name and category; anything else is tidied up as best we can.

/// What kind of spending a transaction is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Category {
    Dining,
    Groceries,
    Transport,
    Shopping,
    Travel,
    Subscriptions,
    Bills,
    Health,
    Leisure,
    Fees,
    Payments,
    Other,
}

impl Category {
    pub fn label(self) -> &'static str {
        match self {
            Category::Dining => "Dining",
            Category::Groceries => "Groceries",
            Category::Transport => "Transport",
            Category::Shopping => "Shopping",
            Category::Travel => "Travel",
            Category::Subscriptions => "Subscriptions",
            Category::Bills => "Bills",
            Category::Health => "Health",
            Category::Leisure => "Leisure",
            Category::Fees => "Fees",
            Category::Payments => "Payments",
            Category::Other => "Other",
        }
    }
}

/// What could be worked out about the other side of a transaction.
#[derive(Clone, Debug, PartialEq)]
pub struct Merchant {
    pub name: String,
    pub category: Category,
    /// Where the transaction was made, e.g. "Seattle, United States", if known.
    pub place: Option<String>,
    /// Payment processor the merchant was paid through, e.g. "PayPal".
    pub via: Option<&'static str>,
}

impl Merchant {
    /// Text the filter box matches against.
    pub fn search_text(&self) -> String {
        format!("{} {} {} {}", self.name, self.category.label(), self.place.as_deref().unwrap_or(""),
                self.via.unwrap_or("")).to_lowercase()
    }
}

use Category::*;

/// Keyword (matched as whole words), display name and category of well-known merchants.
/// More specific keywords come before more general ones.
const KNOWN: &[(&str, &str, Category)] = &[
    // Statement lines
    ("BALANCE PREVIOUS STATEMENT", "Previous balance", Payments),
    ("MONEYSEND", "Card payment", Payments),
    ("PAYMENT THANK YOU", "Card payment", Payments),
    ("GIRO PAYMENT", "GIRO payment", Payments),
    ("CCY CONVERSION FEE", "Currency conversion fee", Fees),
    ("LATE CHARGE", "Late charge", Fees),
    ("FINANCE CHARGE", "Finance charge", Fees),
    ("INTEREST CHARGE", "Interest", Fees),
    ("ANNUAL FEE", "Annual fee", Fees),
    ("CASH ADVANCE", "Cash advance", Fees),
    // Transport
    ("GRABFOOD", "GrabFood", Dining),
    ("GRAB FOOD", "GrabFood", Dining),
    ("GRABMART", "GrabMart", Groceries),
    ("GRAB", "Grab", Transport),
    ("GOJEK", "Gojek", Transport),
    ("TADA", "TADA", Transport),
    ("COMFORTDELGRO", "ComfortDelGro", Transport),
    ("CDG ZIG", "CDG Zig", Transport),
    ("UBER EATS", "Uber Eats", Dining),
    ("UBER", "Uber", Transport),
    ("LYFT", "Lyft", Transport),
    ("SIMPLYGO", "SimplyGo (bus/MRT)", Transport),
    ("TRANSITLINK", "TransitLink", Transport),
    ("EZ LINK", "EZ-Link", Transport),
    ("EZLINK", "EZ-Link", Transport),
    ("SHELL", "Shell", Transport),
    ("ESSO", "Esso", Transport),
    ("CALTEX", "Caltex", Transport),
    ("SPC", "SPC", Transport),
    ("PARKING", "Parking", Transport),
    // Groceries
    ("FAIRPRICE", "NTUC FairPrice", Groceries),
    ("NTUC", "NTUC FairPrice", Groceries),
    ("COLD STORAGE", "Cold Storage", Groceries),
    ("SHENG SIONG", "Sheng Siong", Groceries),
    ("GIANT", "Giant", Groceries),
    ("DON DON DONKI", "Don Don Donki", Groceries),
    ("DONKI", "Don Don Donki", Groceries),
    ("REDMART", "RedMart", Groceries),
    ("SUPERMARKET", "Supermarket", Groceries),
    // Dining
    ("FOODPANDA", "foodpanda", Dining),
    ("DELIVEROO", "Deliveroo", Dining),
    ("MCDONALD", "McDonald's", Dining),
    ("KFC", "KFC", Dining),
    ("BURGER KING", "Burger King", Dining),
    ("MOS BURGER", "MOS Burger", Dining),
    ("JOLLIBEE", "Jollibee", Dining),
    ("SUBWAY", "Subway", Dining),
    ("STARBUCKS", "Starbucks", Dining),
    ("TOAST BOX", "Toast Box", Dining),
    ("YA KUN", "Ya Kun Kaya Toast", Dining),
    ("KOPITIAM", "Kopitiam", Dining),
    ("FOOD REPUBLIC", "Food Republic", Dining),
    ("BREADTALK", "BreadTalk", Dining),
    ("OLD CHANG KEE", "Old Chang Kee", Dining),
    ("DIN TAI FUNG", "Din Tai Fung", Dining),
    ("SAIZERIYA", "Saizeriya", Dining),
    ("GONG CHA", "Gong Cha", Dining),
    ("LIHO", "LiHO", Dining),
    ("CHAGEE", "Chagee", Dining),
    ("PIZZA HUT", "Pizza Hut", Dining),
    ("DOMINO", "Domino's", Dining),
    // Shopping
    ("AMAZON WEB SERVICES", "Amazon Web Services", Subscriptions),
    ("AWS", "Amazon Web Services", Subscriptions),
    ("AMAZON PRIME", "Amazon Prime", Subscriptions),
    ("PRIME VIDEO", "Prime Video", Subscriptions),
    ("AUDIBLE", "Audible", Subscriptions),
    ("AMAZON", "Amazon", Shopping),
    ("AMZN", "Amazon", Shopping),
    ("SHOPEE", "Shopee", Shopping),
    ("LAZADA", "Lazada", Shopping),
    ("TAOBAO", "Taobao", Shopping),
    ("TMALL", "Tmall", Shopping),
    ("ALIEXPRESS", "AliExpress", Shopping),
    ("TEMU", "Temu", Shopping),
    ("QOO10", "Qoo10", Shopping),
    ("EBAY", "eBay", Shopping),
    ("UNIQLO", "Uniqlo", Shopping),
    ("IKEA", "IKEA", Shopping),
    ("MUJI", "MUJI", Shopping),
    ("DAISO", "Daiso", Shopping),
    ("DECATHLON", "Decathlon", Shopping),
    ("COURTS", "Courts", Shopping),
    ("CHALLENGER", "Challenger", Shopping),
    ("BEST DENKI", "Best Denki", Shopping),
    ("HARVEY NORMAN", "Harvey Norman", Shopping),
    ("POPULAR", "Popular", Shopping),
    ("KINOKUNIYA", "Kinokuniya", Shopping),
    ("TAKASHIMAYA", "Takashimaya", Shopping),
    ("ISETAN", "Isetan", Shopping),
    ("TANGS", "Tangs", Shopping),
    ("MUSTAFA", "Mustafa Centre", Shopping),
    ("ZARA", "Zara", Shopping),
    ("H&M", "H&M", Shopping),
    ("COTTON ON", "Cotton On", Shopping),
    ("CHARLES & KEITH", "Charles & Keith", Shopping),
    ("GARMIN", "Garmin", Shopping),
    ("GUMROAD", "Gumroad", Shopping),
    ("APPLE STORE", "Apple Store", Shopping),
    // Subscriptions and online services
    ("APPLE COM BILL", "Apple (App Store/iCloud)", Subscriptions),
    ("ITUNES", "Apple (App Store/iCloud)", Subscriptions),
    ("NETFLIX", "Netflix", Subscriptions),
    ("SPOTIFY", "Spotify", Subscriptions),
    ("DISNEY", "Disney+", Subscriptions),
    ("YOUTUBE", "YouTube", Subscriptions),
    ("GOOGLE", "Google", Subscriptions),
    ("MICROSOFT", "Microsoft", Subscriptions),
    ("MSFT", "Microsoft", Subscriptions),
    ("OPENAI", "OpenAI", Subscriptions),
    ("CHATGPT", "OpenAI", Subscriptions),
    ("ANTHROPIC", "Anthropic", Subscriptions),
    ("CLAUDE AI", "Anthropic", Subscriptions),
    ("ADOBE", "Adobe", Subscriptions),
    ("DROPBOX", "Dropbox", Subscriptions),
    ("GITHUB", "GitHub", Subscriptions),
    ("CLOUDFLARE", "Cloudflare", Subscriptions),
    ("DIGITALOCEAN", "DigitalOcean", Subscriptions),
    ("HETZNER", "Hetzner", Subscriptions),
    ("NAMECHEAP", "Namecheap", Subscriptions),
    ("PATREON", "Patreon", Subscriptions),
    ("PLAYSTATION", "PlayStation", Subscriptions),
    ("STEAM", "Steam", Subscriptions),
    ("STEAMGAMES", "Steam", Subscriptions),
    ("NINTENDO", "Nintendo", Subscriptions),
    ("XBOX", "Xbox", Subscriptions),
    // Bills
    ("SINGTEL", "Singtel", Bills),
    ("STARHUB", "StarHub", Bills),
    ("M1", "M1", Bills),
    ("CIRCLES LIFE", "Circles.Life", Bills),
    ("GOMO", "GOMO", Bills),
    ("MYREPUBLIC", "MyRepublic", Bills),
    ("VIEWQWEST", "ViewQwest", Bills),
    ("SP DIGITAL", "SP Group", Bills),
    ("SP SERVICES", "SP Group", Bills),
    ("SP GROUP", "SP Group", Bills),
    ("GENECO", "Geneco", Bills),
    ("TOWN COUNCIL", "Town Council", Bills),
    ("IRAS", "IRAS", Bills),
    ("AXS", "AXS", Bills),
    ("PRUDENTIAL", "Prudential", Bills),
    ("GREAT EASTERN", "Great Eastern", Bills),
    ("INSURANCE", "Insurance", Bills),
    // Health
    ("WATSONS", "Watsons", Health),
    ("GUARDIAN", "Guardian", Health),
    ("UNITY", "Unity", Health),
    ("RAFFLES MEDICAL", "Raffles Medical", Health),
    ("POLYCLINIC", "Polyclinic", Health),
    ("HOSPITAL", "Hospital", Health),
    ("CLINIC", "Clinic", Health),
    ("DENTAL", "Dental", Health),
    ("MEDICAL", "Medical", Health),
    ("PHARMACY", "Pharmacy", Health),
    // Travel
    ("SINGAPORE AIRLINES", "Singapore Airlines", Travel),
    ("SCOOT", "Scoot", Travel),
    ("JETSTAR", "Jetstar", Travel),
    ("AIRASIA", "AirAsia", Travel),
    ("CATHAY PACIFIC", "Cathay Pacific", Travel),
    ("EMIRATES", "Emirates", Travel),
    ("AGODA", "Agoda", Travel),
    ("BOOKING COM", "Booking.com", Travel),
    ("EXPEDIA", "Expedia", Travel),
    ("AIRBNB", "Airbnb", Travel),
    ("TRIP COM", "Trip.com", Travel),
    ("KLOOK", "Klook", Travel),
    ("HOTEL", "Hotel", Travel),
    // Leisure
    ("SAFRA", "SAFRA", Leisure),
    ("GOLDEN VILLAGE", "Golden Village", Leisure),
    ("SHAW THEATRES", "Shaw Theatres", Leisure),
    ("CATHAY CINEPLEX", "Cathay Cineplexes", Leisure),
    ("SISTIC", "SISTIC", Leisure),
    ("ACTIVESG", "ActiveSG", Leisure),
    ("ANYTIME FITNESS", "Anytime Fitness", Leisure),
    ("FITNESS", "Fitness", Leisure),
    ("GYM", "Gym", Leisure),
];

/// Words that suggest a place to eat when no known merchant matches.
const DINING_WORDS: &[&str] = &[
    "RESTAURANT", "CAFE", "COFFEE", "KOPI", "BAKERY", "BAKE", "KITCHEN", "FOOD", "EAT", "BISTRO", "BAR",
    "SUSHI", "RAMEN", "BBQ", "GRILL", "NOODLE", "DIM SUM", "TEA", "BURGER", "PIZZA", "DINER", "EATERY",
];

/// Payment processors that prefix the real merchant's name, e.g. "PAYPAL *EXAMPLESHOP".
const PROCESSORS: &[(&str, &str)] = &[
    ("PAYPAL", "PayPal"),
    ("SQ", "Square"),
    ("SP", "Shopify"),
    ("TST", "Toast"),
    ("STRIPE", "Stripe"),
    ("2C2P", "2C2P"),
];

const COUNTRIES: &[(&str, &str)] = &[
    ("SG", "Singapore"), ("MY", "Malaysia"), ("ID", "Indonesia"), ("TH", "Thailand"), ("VN", "Vietnam"),
    ("PH", "Philippines"), ("KH", "Cambodia"), ("MM", "Myanmar"), ("HK", "Hong Kong"), ("MO", "Macau"),
    ("CN", "China"), ("TW", "Taiwan"), ("JP", "Japan"), ("KR", "South Korea"), ("IN", "India"),
    ("LK", "Sri Lanka"), ("AU", "Australia"), ("NZ", "New Zealand"), ("US", "United States"),
    ("CA", "Canada"), ("MX", "Mexico"), ("BR", "Brazil"), ("GB", "United Kingdom"), ("IE", "Ireland"),
    ("NL", "Netherlands"), ("BE", "Belgium"), ("LU", "Luxembourg"), ("DE", "Germany"), ("FR", "France"),
    ("IT", "Italy"), ("ES", "Spain"), ("PT", "Portugal"), ("CH", "Switzerland"), ("AT", "Austria"),
    ("SE", "Sweden"), ("NO", "Norway"), ("DK", "Denmark"), ("FI", "Finland"), ("PL", "Poland"),
    ("CZ", "Czechia"), ("EE", "Estonia"), ("CY", "Cyprus"), ("MT", "Malta"), ("TR", "Türkiye"),
    ("IL", "Israel"), ("AE", "United Arab Emirates"), ("SA", "Saudi Arabia"), ("QA", "Qatar"),
];

/// Cities often printed in merchant descriptors, so they can be split off the merchant's name.
const CITIES: &[&str] = &[
    "SINGAPORE", "KUALA LUMPUR", "JOHOR BAHRU", "PETALING JAYA", "PENANG", "BANGKOK", "JAKARTA", "BATAM",
    "BALI", "MANILA", "HO CHI MINH", "HANOI", "HONG KONG", "TAIPEI", "TOKYO", "OSAKA", "KYOTO", "SEOUL",
    "SHANGHAI", "BEIJING", "SHENZHEN", "HANGZHOU", "GUANGZHOU", "SYDNEY", "MELBOURNE", "PERTH",
    "AUCKLAND", "LONDON", "SOUTHAMPTON", "MANCHESTER", "DUBLIN", "AMSTERDAM", "LUXEMBOURG", "PARIS",
    "BERLIN", "MUNICH", "ZURICH", "STOCKHOLM", "NEW YORK", "SEATTLE", "SAN FRANCISCO", "SAN JOSE",
    "MOUNTAIN VIEW", "CUPERTINO", "LOS GATOS", "MENLO PARK", "PALO ALTO", "REDMOND", "LOS ANGELES",
    "BOSTON", "TORONTO", "VANCOUVER", "DUBAI",
];

/// Splits text into upper-case words, so "Apple.com/bill" and "APPLE COM BILL" compare equal.
fn words(text: &str) -> Vec<String> {
    text.to_uppercase()
        .split(|c: char| !(c.is_alphanumeric() || c == '&'))
        .filter(|w| !w.is_empty())
        .map(str::to_string)
        .collect()
}

/// Position of `needle` as a run of whole words within `haystack`.
fn find_words(haystack: &[String], needle: &[String]) -> Option<usize> {
    if needle.is_empty() || needle.len() > haystack.len() {
        return None;
    }
    (0..=haystack.len() - needle.len()).find(|&i| haystack[i..i + needle.len()] == *needle)
}

fn contains_words(haystack: &[String], needle: &str) -> bool {
    find_words(haystack, &words(needle)).is_some()
}

/// "GARMIN Europe AFP" -> "Garmin Europe AFP": capitalises words, keeping short all-caps
/// abbreviations (no vowels) as they are.
fn title_case(text: &str) -> String {
    text.split_whitespace()
        .map(|word| {
            let is_abbreviation = word.len() <= 4 && !word.chars().any(|c| "AEIOUaeiou".contains(c));
            if is_abbreviation || word.chars().any(|c| c.is_ascii_digit()) {
                return word.to_string();
            }
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().chain(chars.flat_map(char::to_lowercase)).collect(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Works out the merchant, category and place of a transaction from its description.
pub fn identify(description: &str) -> Merchant {
    let mut tokens: Vec<&str> = description.split_whitespace().collect();

    // Trailing two-letter country code.
    let country = tokens.last()
        .and_then(|last| COUNTRIES.iter().find(|(code, _)| last.eq_ignore_ascii_case(code)))
        .map(|&(_, name)| name);
    if country.is_some() && tokens.len() > 1 {
        tokens.pop();
    }

    // City just before the country, if it's one we know.
    let mut city = None;
    if country.is_some() {
        let upper: Vec<String> = tokens.iter().map(|t| t.to_uppercase()).collect();
        for known in CITIES {
            let city_words = words(known);
            if upper.len() > city_words.len() && upper.ends_with(&city_words) {
                tokens.truncate(tokens.len() - city_words.len());
                city = Some(title_case(known));
                break;
            }
        }
    }
    let place = match (city, country) {
        (Some(city), Some(country)) if city != country => Some(format!("{city}, {country}")),
        (_, Some(country)) => Some(country.to_string()),
        (city, None) => city,
    };

    // A payment processor in front of the merchant, e.g. "PAYPAL *EXAMPLESHOP".
    let mut via = None;
    if let Some(first) = tokens.first() {
        let head = first.split('*').next().unwrap_or("");
        if first.contains('*') || tokens.get(1).is_some_and(|t| t.starts_with('*')) {
            if let Some(&(_, name)) = PROCESSORS.iter().find(|(p, _)| head.eq_ignore_ascii_case(p)) {
                via = Some(name);
                let rest = first.split_once('*').map(|(_, r)| r).unwrap_or("");
                if rest.is_empty() {
                    tokens.remove(0);
                } else {
                    tokens[0] = rest;
                }
            }
        }
    }
    for token in tokens.iter_mut() {
        *token = token.trim_matches('*');
    }
    tokens.retain(|t| !t.is_empty());

    let all_words = words(description);
    let rest = tokens.join(" ");
    let rest_words = words(&rest);

    if let Some(&(_, name, category)) = KNOWN.iter().find(|(keyword, _, _)| contains_words(&all_words, keyword)) {
        return Merchant { name: name.to_string(), category, place, via };
    }

    // Unknown merchant: drop reference numbers and legal suffixes, and tidy up what's left.
    let mut name_tokens: Vec<&str> = tokens.iter().copied()
        .filter(|t| t.chars().filter(|c| c.is_ascii_digit()).count() < 4)
        .filter(|t| !["PTE", "LTD", "PTE.", "LTD.", "INC", "INC.", "LLC", "CO.", "SDN", "BHD"]
            .contains(&t.to_uppercase().as_str()))
        .collect();
    // Descriptors often repeat a cut-off copy of the name, e.g. "EXAMPLESHOP EXAMPL".
    name_tokens.dedup_by(|next, prev| prev.len() >= 4 && next.len() >= 4
        && prev.to_uppercase().starts_with(&next.to_uppercase()));
    let name = if name_tokens.is_empty() { title_case(&rest) } else { title_case(&name_tokens.join(" ")) };
    let category = if DINING_WORDS.iter().any(|w| contains_words(&rest_words, w)) { Dining } else { Other };
    Merchant { name, category, place, via }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognises_known_merchants() {
        let m = identify("AMAZON MKTPLACE PMTS SEATTLE US");
        assert_eq!(m.name, "Amazon");
        assert_eq!(m.category, Shopping);
        assert_eq!(m.place.as_deref(), Some("Seattle, United States"));

        let m = identify("UNIQLO Europe LONDON GB");
        assert_eq!((m.name.as_str(), m.category), ("Uniqlo", Shopping));
        assert_eq!(m.place.as_deref(), Some("London, United Kingdom"));

        assert_eq!(identify("SHOPEE SG MP SINGAPORE SG").place.as_deref(), Some("Singapore"));
        assert_eq!(identify("MONEYSEND JOHN DOE SINGAPORE SG").category, Payments);
        assert_eq!(identify("CCY CONVERSION FEE SGD 100.00").category, Fees);
        assert_eq!(identify("BALANCE PREVIOUS STATEMENT").name, "Previous balance");
        assert_eq!(identify("APPLE.COM/BILL ITUNES.COM SG").category, Subscriptions);
    }

    #[test]
    fn matches_whole_words_only() {
        // "SPC" must not match inside other words, nor "GRAB" inside "GRABFOOD".
        assert_eq!(identify("GRABFOOD SINGAPORE SG").name, "GrabFood");
        assert_ne!(identify("ESPCLUB SINGAPORE SG").name, "SPC");
    }

    #[test]
    fn unwraps_payment_processors() {
        let m = identify("PAYPAL *EXAMPLESHOP EXAMPL 4155550100 US");
        assert_eq!(m.via, Some("PayPal"));
        assert_eq!(m.name, "Exampleshop");
        assert_eq!(m.place.as_deref(), Some("United States"));
    }

    #[test]
    fn guesses_dining_from_the_name() {
        let m = identify("CAFE @ CENTRAL SINGAPORE SG");
        assert_eq!(m.category, Dining);
        assert_eq!(m.name, "Cafe @ Central");
        assert_eq!(identify("SAIZERIYA - CENTRAL SINGAPORE SG").category, Dining);
    }
}
