#![feature(vec_into_raw_parts)]
use libc::*;
use nakama_rs::*;

use std::sync::{Arc, Mutex};
use lazy_static::*;

pub struct NakamaClient {
    client: NClient,
}

unsafe impl Send for NakamaClient {}

impl NakamaClient {
    pub fn new(server_key: &str, host: &str, port: u16, ssl: bool) -> Self {
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

unsafe impl Send for NakamaRealtimeClient {}

impl NakamaRealtimeClient {
    pub fn new(client: &mut NakamaClient, port: u16) -> Self {
        let rt_client = unsafe{NClient_createRtClient(client.client, port.into())};
        unsafe{NRtClient_setErrorCallback(rt_client, Some(realtime_error_callback));};
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
        } else {
            println!("Can't connect RtClient because Client isn't connected!");
        }
    }

    pub fn is_connected(&self) -> bool {
        if(unsafe{NRtClient_isConnected(self.rt_client)} == 1) {
            true
        } else {
            false
        }
    }

    pub fn match_make(&mut self) {
        let query = std::ffi::CString::new("*").expect("CString::new failed");
        let string_props = unsafe{NStringMap_create()};
        let double_props = unsafe{NStringDoubleMap_create()};
        unsafe{
            NRtClient_setMatchmakerMatchedCallback(self.rt_client, Some(matchmaking_completed));
            self.tick();
            NRtClient_setMatchDataCallback(self.rt_client, Some(match_data_receive));
            self.tick();
            NRtClient_addMatchmaker(
                self.rt_client,
                2,
                2,
                query.as_ptr(),
                string_props,
                double_props,
                std::ptr::null_mut(),
                Some(matchmaking_success),
                Some(realtime_error_callback_match),
            );
            self.tick();
        }
    }
}

extern "C" fn matchmaking_success(_client: NRtClient, _: *mut libc::c_void, ticket: *const NMatchmakerTicket) {
    println!("joining matchmaking success!");
}

extern "C" fn realtime_error_callback_match(client: NRtClient, _: NRtClientReqData, err: *const sNRtError) {
    println!("error in matchmaking!");
    realtime_error_callback(client, err);
}

extern "C" fn realtime_error_callback(_client: NRtClient, err: *const sNRtError) {
    let msg = unsafe{std::ffi::CStr::from_ptr((*err).message)};
    println!("rtclient error callback called: {}", msg.to_str().unwrap());
}

extern "C" fn client_error_callback(_client: NClient, _: *mut libc::c_void, err: *const sNError) {
    let msg = unsafe{std::ffi::CStr::from_ptr((*err).message)};
    println!("client error callback called: {}", msg.to_str().unwrap());
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
    game: NMatch,
    match_id: String,
}

unsafe impl Send for NakamaMatch {}

impl NakamaMatch {
    pub fn send_data(&mut self, rt_client: &mut NakamaRealtimeClient, opcode: i64, mut data: Vec<u8>) {
        data.shrink_to_fit();
        /*let len = data.len();
        let boxed_data = Box::new(data);
        let data_ptr = data.as_mut_ptr();*/
        let (data_ptr, len, _) = data.into_raw_parts();
        assert!(!data_ptr.is_null());
        let bytes = Box::into_raw(Box::new(NBytes {
            bytes: data_ptr,
            size: len as u32,
        }));
        //assert!(!self.game.is_null());
        unsafe {
            assert!(!(self.game).matchId.is_null());
            //assert!(!(self.game).presences.is_null());
            println!("Sending data unsafe!");
            println!("Presences count: {}", (self.game).presencesCount);
            //let msg = unsafe{std::ffi::CStr::from_ptr((self.game).matchId)};
            //println!("match id is again: {}", msg.to_str().unwrap());
                //NRtClient_sendMatchData(
                //    rt_client.rt_client,
                //    (self.game).matchId,
                //    opcode,
                //    bytes,
                //    (self.game).presences,
                //    (self.game).presencesCount);
            let copy = Box::into_raw(Box::new(self.match_id.clone()));
            let ptr = std::ffi::CString::new((*copy).as_str()).unwrap();
                NRtClient_sendMatchData(
                    rt_client.rt_client,
                    ptr.as_ptr(),
                    opcode,
                    bytes,
                    std::ptr::null_mut(),
                    0);
        }
        println!("Sending data tick!");
        rt_client.tick();
    }
}

extern "C" fn match_joined(client: NRtClient, _: *mut libc::c_void, game: *const sNMatch) {
    println!("joined match!");
    unsafe {
        let msg = unsafe{std::ffi::CStr::from_ptr((*game).matchId)};
        let msg_str = String::from(msg.to_str().unwrap().clone());
        *MATCH.lock().unwrap() = Some(NakamaMatch{game: *game.clone(), match_id: msg_str});
    }
}

extern "C" fn match_data_receive(client: NRtClient, data: *const NMatchData) {
    unsafe {
        let opcode = (*data).opCode;
        let bytes = (*data).data;
        let bytes = Vec::from_raw_parts(bytes.bytes, bytes.size as usize, bytes.size as usize);
        let bytes2 = bytes.clone();
        // Just leak that memory. It should get cleaned up by the cpp sdk.
        // TODO check that it does indeed get cleaned up.
        Box::into_raw(Box::new(bytes));
        RECEIVED_DATA.lock().unwrap().push((opcode, bytes2));
    }
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
    pub static ref LAST_AUTH: Mutex<Option<NakamaSession>> = Mutex::new(None);
    pub static ref MATCH: Mutex<Option<NakamaMatch>> = Mutex::new(None);
    pub static ref RECEIVED_DATA: Mutex<Vec<(i64, Vec<u8>)>> = Mutex::new(Vec::new());
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
    unsafe{NClient_authenticateEmail(client.client, email.as_ptr(), password.as_ptr(), username.as_ptr(), 1, opts, std::ptr::null_mut(), Some(logged_in), Some(client_error_callback));}
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
