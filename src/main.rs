use std::env;
use std::convert::TryFrom;
use regex::Regex;

const REGEX_STEAMID2: &str = r"^STEAM_([0-5]):([01]):(\d+$)";
const REGEX_STEAMID3: &str = r"^\[(.):([01]):(\d+)\]$";

/* Valve SteamID Format:
 *  A SteamID is just a packed 64-bit unsigned integer!
 * 
 * It consists of five parts, from least to most significant bit:
 *  1. Authentication Server    - 1 bit     (1)
 *  2. Account Number           - 31 bits   (32)
 *  3. Instance                 - 20 bits   (52)
 *  4. Account Type             - 4 bits    (56)
 *  5. Universe                 - 8 bits    (64)
 * 
 * This can be visualized like so:
 *  1. _______________________________________________________________X
 *  2. ________________________________XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX_
 *  3. ____________XXXXXXXXXXXXXXXXXXXX________________________________
 *  4. ________XXXX____________________________________________________
 *  5. XXXXXXXX________________________________________________________
 * 
 * There are multiple ways to express a SteamID, some are lossy.
 *  A. steamID64        - (1)+(2)+(3)+(4)+(5)
 *  B. steamID2         - STEAM_(5):(1):(2)
 *  C. steamID3         - [(4):(5):(1)+(2)]
*/

/*
 * Instance
 *  The Instance field nominally holds what 'instance' the steamID is, however
 *  when specifying a chatroom, the last 8 bits define the "type" of chatroom.
 * This can be visualized like so:
 *  ____________ZZZZZZZZXXXXXXXXXXXX
*/

fn account_type_to_char(account_type: u8, instance: Option<u32>) -> char {
    match account_type {
        0   => 'I', // Invalid
        1   => 'U', // Individual (Profiles)
        2   => 'M', // Multiseat
        3   => 'G', // GameServer
        4   => 'A', // AnonGameServer
        5   => 'P', // Pending
        6   => 'C', // ContentServer
        7   => 'g', // Clan (Groups)
        8   =>  match instance.unwrap_or(0) >> 12 & 255 { // EChatSteamIDInstanceFlags (Shifted 12 right)
                    1   => 'T', // MatchMaking Lobby
                    2   => 'L', // Lobby
                    4   => 'c', // Clan Chat
                    _   => 'c', // Clan Chat (Default)
                }
        //9   => '',// ConsoleUser
        10  => 'a', // AnonUser
        _   => 'I', // Invalid (Default)
    }
}

#[derive(Debug)]
enum SteamIDAccountType {
    Invalid,
    Individual,
    Multiseat,
    GameServer,
    AnonGameServer,
    Pending,
    ContentServer,
    Clan,
    Chat,
    ConsoleUser,
    AnonUser,
}

impl SteamIDAccountType {
    fn to_char(self, instance: Option<u32>) -> char {
        match self {
            SteamIDAccountType::Invalid         => 'I',
            SteamIDAccountType::Individual      => 'U',
            SteamIDAccountType::Multiseat       => 'M',
            SteamIDAccountType::GameServer      => 'G',
            SteamIDAccountType::AnonGameServer  => 'A',
            SteamIDAccountType::Pending         => 'P',
            SteamIDAccountType::ContentServer   => 'C',
            SteamIDAccountType::Clan            => 'g',
            SteamIDAccountType::Chat            =>
                    match instance.unwrap_or(0) >> 12 & 255 { // EChatSteamIDInstanceFlags (Shifted 12 right)
                        1   => 'T', // MatchMaking Lobby
                        2   => 'L', // Lobby
                        4   => 'c', // Clan Chat
                        _   => 'c', // Clan Chat (Default)
                    }
            SteamIDAccountType::ConsoleUser     => 'I',
            SteamIDAccountType::AnonUser        => 'a',
        }
    }
    fn to_int(self) -> u32 {
        match self {
            SteamIDAccountType::Invalid         => 0,
            SteamIDAccountType::Individual      => 1,
            SteamIDAccountType::Multiseat       => 2,
            SteamIDAccountType::GameServer      => 3,
            SteamIDAccountType::AnonGameServer  => 4,
            SteamIDAccountType::Pending         => 5,
            SteamIDAccountType::ContentServer   => 6,
            SteamIDAccountType::Clan            => 7,
            SteamIDAccountType::Chat            => 8,
            SteamIDAccountType::ConsoleUser     => 9,
            SteamIDAccountType::AnonUser        => 10,
        }
    }
    fn from_char(account_type: char) -> SteamIDAccountType {
        match account_type {
            'I'   => SteamIDAccountType::Invalid,
            'U'   => SteamIDAccountType::Individual,
            'M'   => SteamIDAccountType::Multiseat,
            'G'   => SteamIDAccountType::GameServer,
            'A'   => SteamIDAccountType::AnonGameServer,
            'P'   => SteamIDAccountType::Pending,
            'C'   => SteamIDAccountType::ContentServer,
            'g'   => SteamIDAccountType::Clan,
            'c'   => SteamIDAccountType::Chat,
            'T'   => SteamIDAccountType::Chat,
            'L'   => SteamIDAccountType::Chat,
            'a'   => SteamIDAccountType::AnonUser,
            _     => SteamIDAccountType::Invalid,
        }
    }
    fn from_int(account_type: u8) -> SteamIDAccountType {
        match account_type {
            0   => SteamIDAccountType::Invalid,
            1   => SteamIDAccountType::Individual,
            2   => SteamIDAccountType::Multiseat,
            3   => SteamIDAccountType::GameServer,
            4   => SteamIDAccountType::AnonGameServer,
            5   => SteamIDAccountType::Pending,
            6   => SteamIDAccountType::ContentServer,
            7   => SteamIDAccountType::Clan,
            8   => SteamIDAccountType::Chat,
            9   => SteamIDAccountType::ConsoleUser,
            10  => SteamIDAccountType::AnonUser,
            _   => SteamIDAccountType::Invalid,
        }
    }
}

#[derive(Debug, PartialEq)]
enum SteamIDFormat{
    SteamID64,
    SteamID2,
    SteamID3,
}

fn string_to_steamid_type(steamid: &str) -> Result<SteamIDFormat, &str> {
    if steamid.parse::<u64>().is_ok() {
        return Ok(SteamIDFormat::SteamID64)
    }
    let steamid2 = Regex::new(REGEX_STEAMID2).unwrap();
    if steamid2.is_match(steamid) {
        return Ok(SteamIDFormat::SteamID2)
    }
    
    let steamid3 = Regex::new(REGEX_STEAMID3).unwrap();
    if steamid3.is_match(steamid) {
        return Ok(SteamIDFormat::SteamID3)
    }
    Err("Unable to parse to any SteamID Format.")
}

fn steamid2_to_steamid64( steamid2: &str) -> u64 {
    let regex = Regex::new(REGEX_STEAMID2).unwrap();
    let captures = regex.captures(steamid2).unwrap();

    let universe = captures.get(1).unwrap().as_str().parse::<u64>().unwrap_or(1);
    let auth_server = captures.get(2).unwrap().as_str().parse::<u64>().unwrap();
    let account_id = captures.get(3).unwrap().as_str().parse::<u64>().unwrap();

    let steam64 = (universe << 56) | auth_server | (account_id << 1) | 76561197960265728;
    steam64
}

fn steamid3_to_steamid64( steamid3: &str) -> u64 {
    let regex = Regex::new(REGEX_STEAMID3).unwrap();
    let captures = regex.captures(steamid3).unwrap();

    let account_type = captures.get(1).unwrap().as_str().parse::<char>().unwrap_or('I');
    let account_type = u64::from(SteamIDAccountType::to_int(SteamIDAccountType::from_char(account_type)));

    let universe = captures.get(2).unwrap().as_str().parse::<u64>().unwrap_or(1);
    let account_id = captures.get(3).unwrap().as_str().parse::<u64>().unwrap();

    let steam64 = (account_type << 52) | (universe << 56) | account_id ;
    steam64
}

fn string_to_steamid64( input: &str) -> Result<u64, &str> {
    let steam_type = string_to_steamid_type(input);
    match steam_type {
        Ok(steam_type) => {
            match steam_type {
                SteamIDFormat::SteamID64 => return Ok(input.parse::<u64>().unwrap()),
                SteamIDFormat::SteamID2 => return Ok(steamid2_to_steamid64(input)),
                SteamIDFormat::SteamID3 => return Ok(steamid3_to_steamid64(input)),
            }
        }
        Err(steam_type) => return Err(steam_type)
    }
}

struct SteamID {
    account_id: u32,
    account_instance: u32,
    account_type: u8,
    account_universe: u8,
}

impl SteamID {
    fn new() -> SteamID {
        SteamID {
            account_id: 0,
            account_instance:  1,
            account_type: 1,
            account_universe: 1,
        }
    }
    fn set_steamid64(&mut self, steamid_64: u64) {
        self.account_id = u32::try_from(steamid_64 & 4294967295).unwrap_or(1);
        self.account_instance =  u32::try_from(steamid_64 >> 32 & 1048575).unwrap_or(1);
        self.account_type = u8::try_from(steamid_64 >> 52 & 15).unwrap_or(1);
        self.account_universe = u8::try_from(steamid_64 >> 56 & 15).unwrap_or(1);
    }
    fn get_steamid64(&self) -> u64 {
        u64::from(self.account_id) |
        u64::from(self.account_instance) << 32 |
        u64::from(self.account_type) << 52 |
        u64::from(self.account_universe)  << 56
    }
    fn get_steamid2(&self) -> String {
        let authserver: u32 = self.account_id & 1; // Ideally we'd cast this to a bool and convert that to a 0 or 1 later.
        let accountid: u32 = (self.account_id >> 1) & 2147483647;
        format!("STEAM_{}:{}:{}", self.account_universe, authserver, accountid)
    }
    fn get_steamid3(&self) -> String {
        let type_char: char = account_type_to_char(self.account_type, Some(self.account_instance));
        format!("[{}:{}:{}]", type_char, self.account_universe, self.account_id)
    }
}



fn main() {
    // Gather our CLI arguments
    let args: Vec<String> = env::args().collect();

    //println!("Args: {:?}", args);
    //println!("Len: {}", args.len());
   
    // Dumb check, make sure they even tried providing a SteamID
    if args.len() < 2 {
        println!("No IDs provided!");
        std::process::exit(-1);
    }

    // Process all of our passed strings
    for i in 1..args.len() {
        let input = &args[i];
        let steam_type = string_to_steamid_type(input);
        match steam_type {
            Ok(steam_type) => {
                println!("Interpreting as {:?}", steam_type );
                match steam_type {
                    SteamIDFormat::SteamID64 => {
                        let mut steamid_object = SteamID::new();
                        steamid_object.set_steamid64(input.parse::<u64>().expect("SteamID64 Not a Number!"));
                        println!("steamID64:\t{}", steamid_object.get_steamid64());
                        println!("steamID:  \t{}", steamid_object.get_steamid2());
                        println!("steamID3: \t{}", steamid_object.get_steamid3());
                    }
                    SteamIDFormat::SteamID2 => {
                        let mut steamid_object = SteamID::new();
                        steamid_object.set_steamid64(steamid2_to_steamid64(input));
                        println!("steamID64:\t{}", steamid_object.get_steamid64());
                        println!("steamID:  \t{}", steamid_object.get_steamid2());
                        println!("steamID3: \t{}", steamid_object.get_steamid3());
                    }
                    SteamIDFormat::SteamID3 => {
                        let mut steamid_object = SteamID::new();
                        steamid_object.set_steamid64(steamid3_to_steamid64(input));
                        println!("steamID64:\t{}", steamid_object.get_steamid64());
                        println!("steamID:  \t{}", steamid_object.get_steamid2());
                        println!("steamID3: \t{}", steamid_object.get_steamid3());
                    }
                }
            }
            Err(_steam_type) => {
                println!("Unable to interpret {}", input);
            }
        }
        println!("");
    }

    // println!("Generating Alias SteamID64s...");
    // thread::sleep(time::Duration::from_secs(1));
    // for n in 1..1048575 {
    //     let instance: u64 = n << 32;
    //     let newsteam64 = (steamid64 & 18442240478377148415) | instance;
    //     println!("http://steamcommunity.com/profiles/{}", newsteam64);
    // }
}
