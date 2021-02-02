use libc::*;
use nakama_rs::*;

use std::sync::{Arc, Mutex};
use lazy_static::*;

pub struct NakamaClient {
    client: NClient,
}

impl NakamaClient {
    pub fn new(server_key: &'static str, host: &'static str, port: u16, ssl: bool) -> Self {
        let server_key = std::ffi::CString::new(server_key).expect("CString::new failed");
        let host = std::ffi::CString::new(host).expect("CString::new failed");
        let params = tNClientParameters {
            serverKey: server_key.as_ptr(),
            host: host.as_ptr(),
            port: port.into(),
            ssl: if ssl {1} else {0},
        };
        let client = unsafe{createDefaultNakamaClient(&params)};
        Self {
            client
        }
    }

    pub fn tick(&mut self) {
        unsafe{NClient_tick(self.client);}
    }
}

impl Drop for NakamaClient {
    fn drop(&mut self) {
        unsafe {destroyNakamaClient(self.client)};
    }
}

pub struct NakamaRealtimeClient {
    rt_client: NRtClient,
}

impl NakamaRealtimeClient {
    pub fn new(client: &mut NakamaClient, port: u16) -> Self {
        let rt_client = unsafe{NClient_createRtClient(client.client, port.into())};
        Self {
            rt_client
        }
    }

    pub fn tick(&mut self) {
        unsafe{NRtClient_tick(self.rt_client);}
    }

    pub fn connect(&mut self) {
        if let Some(session) = &*LAST_AUTH.lock().unwrap() {
            unsafe{NRtClient_connect(self.rt_client, session.session, 1, NRtClientProtocol_NRtClientProtocol_Json);}
        }
    }

    pub fn match_make(&mut self) {
        let query = std::ffi::CString::new("*").expect("CString::new failed");
        let string_props = unsafe{NStringMap_create()};
        let double_props = unsafe{NStringDoubleMap_create()};
        unsafe{
            NRtClient_setMatchmakerMatchedCallback(self.rt_client, Some(matchmaking_completed));
            NRtClient_setMatchDataCallback(self.rt_client, Some(match_data_receive));
            NRtClient_addMatchmaker(
                self.rt_client,
                2,
                2,
                query.as_ptr(),
                string_props,
                double_props,
                std::ptr::null_mut(),
                Some(matchmaking_success),
                None,
            );
        }
    }
}

extern "C" fn matchmaking_success(client: NRtClient, _: *mut libc::c_void, ticket: *const NMatchmakerTicket) {
    println!("joining matchmaking success!");
}

extern "C" fn matchmaking_completed(client: NRtClient, matched: *const sNMatchmakerMatched) {
    unsafe{
        NRtClient_joinMatchByToken(
            client, 
            (*matched).token,
            std::ptr::null_mut(),
            Some(match_joined),
            None,
            );
    }
    println!("matchmaking completed!");
}

pub struct NakamaMatch {
    game: *const NMatch,
}

unsafe impl Send for NakamaMatch {}

impl NakamaMatch {
    pub fn send_data(&mut self, rt_client: &mut NakamaRealtimeClient, opcode: i64, mut data: Vec<u8>) {
        let mut bytes = NBytes {
            bytes: data.as_mut_ptr(),
            size: data.len() as u32,
        };
        unsafe {
            NRtClient_sendMatchData(rt_client.rt_client, (*self.game).matchId, opcode, &bytes, (*self.game).presences, (*self.game).presencesCount);
        }
    }
}

extern "C" fn match_joined(client: NRtClient, _: *mut libc::c_void, game: *const sNMatch) {
    println!("joined match!");
    *MATCH.lock().unwrap() = Some(NakamaMatch{game});
}

extern "C" fn match_data_receive(client: NRtClient, data: *const NMatchData) {
    // TODO
    // match opcode
}

impl Drop for NakamaRealtimeClient {
    fn drop(&mut self) {
        unsafe {NRtClient_destroy(self.rt_client);};
    }
}

pub fn enable_debug_logs() {
    unsafe{NLogger_initWithConsoleSink(NLogLevel__NLogLevel_Debug);}
}

lazy_static! {
    static ref LAST_AUTH: Mutex<Option<NakamaSession>> = Mutex::new(None);
    static ref MATCH: Mutex<Option<NakamaMatch>> = Mutex::new(None);
}

pub struct NakamaSession {
    pub session: NSession,
}
unsafe impl Send for NakamaSession {}

pub fn auth_email(client: &mut NakamaClient, email: &'static str, password: &'static str, username: &'static str) {
    let email = std::ffi::CString::new(email).expect("CString::new failed");
    let password = std::ffi::CString::new(password).expect("CString::new failed");
    let username = std::ffi::CString::new(username).expect("CString::new failed");
    let opts = unsafe{NStringMap_create()};
    unsafe{NClient_authenticateEmail(client.client, email.as_ptr(), password.as_ptr(), username.as_ptr(), 1, opts, std::ptr::null_mut(), Some(logged_in), None);}
}

extern "C" fn logged_in(client: *mut NClient_, _: *mut libc::c_void, session: *mut NSession_) {
    *LAST_AUTH.lock().unwrap() = Some(NakamaSession{session});
}

#[cfg(test)]
mod tests {
    use crate::*;
    #[test]
    fn it_works() {
        enable_debug_logs();
        let mut client = NakamaClient::new("defaultkey", "127.0.0.1", 7350, false);

        //let _rt_client = NakamaRealtimeClient::new(&mut client, 7351);
        client.tick();
        client.tick();
        client.tick();
        client.tick();
        client.tick();
        std::thread::sleep_ms(2000);
        client.tick();
        client.tick();
        //auth_email(&mut client, "jojolepro@jojolepro.com", "no_uuuuu", "jojolepro");
        auth_email(&mut client, "email@example.com", "3bc8f72e95a9aaa", "mycustomusername");
        client.tick();
        client.tick();
        std::thread::sleep_ms(2000);
        client.tick();
        client.tick();
        client.tick();
        assert!(LAST_AUTH.lock().unwrap().is_some());
        //NClient_authenticateEmail(client.client, 
        //let x: NSessionCallback = 1;
        assert_eq!(2 + 2, 4);
    }
}
