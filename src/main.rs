mod response_creator;
use crate::response_creator::*;

mod waiter;
use crate::waiter::*;

use std::{
    env, mem,
    sync::Arc,
    time::{Duration, SystemTime},
};

use parking_lot::{Mutex, Condvar};

use rusqlite::{params, Connection, Row};

use lazy_static::lazy_static;

extern crate ctrlc;

type SqliteResult<T> = rusqlite::Result<T>;
type SqliteError = rusqlite::Error;

use serenity::{
    builder::CreateMessage,
    framework::standard::{
        macros::{command, group},
        CommandResult, StandardFramework,
    },
    model::{
        channel::{Message, PrivateChannel, Reaction},
        gateway::{Activity, Ready},
        id::{GuildId, MessageId, RoleId, UserId},
    },
    prelude::*,
};

#[group]
#[commands(ping)]
struct UserManagement;

lazy_static! {
    pub(crate) static ref DB: Mutex<Connection> = {
        let db_file = env::var("DB_FILE")
            .expect("Error while reading DB_FILE! Set the environment variable.");

        Mutex::from(
            Connection::open(db_file).expect("Error while reading database.")
        )
    };

    pub(crate) static ref GUEST_CONDVAR_MUTEX: Mutex<()> = Mutex::default();
    pub(crate) static ref GUEST_CONDVAR: Condvar = Condvar::default();
}

// Simple wrapper to avoid ugly unsafe blocks

struct Handler;
impl EventHandler for Handler {
    fn ready(&self, ctx: Context, ready: Ready) {
        eprintln!("{} ({}) is connected!", ready.user.name, ready.user.id);
        ctx.set_activity(Activity::playing("with cat toys")); // TODO: Database entry for this
    }

    fn reaction_add(&self, mut ctx: Context, rxn: Reaction) {
        let current_user = ctx.http
            .get_current_user()
            .expect("Error while getting user info.");
        if current_user.id == rxn.user_id {
            return;
        }

        let guild_id = match rxn.guild_id {
            Some(x) => x,
            None => return,
        };

        let s_gid = guild_id.0 as i64;

        let result = {
            let db_lock = DB.lock();

            let mut stmt = db_lock.prepare("SELECT message_id, role_id, guest_duration FROM guild_configs WHERE guild_id = ?1").expect("Prepare failed.");
            stmt.query_row(
                params![s_gid],
                |row: &Row| -> SqliteResult<(MessageId, RoleId, Duration)> {
                    Ok((
                        MessageId::from(row.get::<_, i64>(0)? as u64),
                        RoleId::from(row.get::<_, i64>(1)? as u64),
                        Duration::from_secs(row.get::<_, i64>(2)? as u64),
                    ))
                },
            )
        };

        if let Ok(x) = result {
            if x.0 == rxn.message_id {
                if let Err(e) = rxn.delete(&ctx) {
                    eprintln!("Couldn't delete reaction: {:?}", e);
                }

                let mut channel: PrivateChannel = rxn.user_id
                    .create_dm_channel(&ctx)
                    .expect("Error while creating DM channel.");

                execute_guest(&mut ctx, rxn.user_id, guild_id, x.1, &mut channel, x.2);
            }
        }
    }
}

fn invalid_command(ctx: &mut Context, msg: &Message, cmd: &str) {
    if let Err(error) = msg
        .channel_id
        .say(&ctx, format!("Command `{}` not found.", cmd))
    {
        eprintln!(
            "Error '{:?}' while sending reply in {}.",
            &error, msg.channel_id
        );
    }
}

fn execute_guest(
    ctx: &mut Context,
    uid: UserId,
    gid: GuildId,
    rid: RoleId,
    ch: &mut PrivateChannel,
    dur: Duration,
) {
    let resp_first = check_guest_presence(ctx, uid, gid);

    let resp = resp_first.unwrap_or_else(|| assign_guest(ctx, uid, gid, rid, dur));

    if let Err(error) = ch.send_message(ctx, |msg: &mut CreateMessage| resp.send(msg)) {
        eprintln!("Error '{}' while sending priate message.", &error);
    }
}

// Checks presence of guest role on a user and returns None when the user is eligible for one. Returns Some(_) with response to send back to the user.
fn check_guest_presence(ctx: &mut Context, uid: UserId, gid: GuildId) -> Option<GuestResponse> {
    if gid
        .member(&ctx, uid)
        .expect("Error while getting member.")
        .roles(&ctx)
        .map_or(false, |vec| !vec.is_empty())
    {
        return Some(GuestResponse::AlreadyHasMember);
    }

    let s_gid = gid.0 as i64;
    let s_uid = uid.0 as i64;

    let db_lock = DB.lock();

    let result = {
        let mut stmt = db_lock
            .prepare("SELECT timestamp FROM guests WHERE guild_id = ?1 AND user_id = ?2")
            .expect("Error while preparing statement.");
        stmt.query_row(
            params![s_gid, s_uid],
            |row: &Row| -> SqliteResult<SystemTime> {
                Ok(
                    SystemTime::UNIX_EPOCH + Duration::from_secs(row.get::<_, i64>(0)? as u64)
                )
            },
        )
    };

    mem::drop(db_lock); // Explicit drop if we ever need to extend this function

    match result {
        Err(x) => {
            if let SqliteError::QueryReturnedNoRows = x {
                None
            } else {
                eprintln!("Error '{:?}' while reading database.", x);
                Some(GuestResponse::InternalError)
            }
        }
        Ok(output) => Some(GuestResponse::present(output)),
    }
}

fn assign_guest(
    ctx: &mut Context,
    uid: UserId,
    gid: GuildId,
    rid: RoleId,
    dur: Duration,
) -> GuestResponse {
    let mut member = gid.member(&ctx, uid).expect("Error while getting member.");

    let expiration = SystemTime::now() + dur;

    {
        let db_lock = DB.lock();

        let mut stmt = db_lock
            .prepare("INSERT INTO guests (guild_id, user_id, timestamp) VALUES (?1, ?2, ?3)")
            .expect("Error while creating statement.");

        let timestamp = expiration
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("Error while creating timestamp.")
            .as_secs()
            as i64;

        let s_gid = gid.0 as i64;
        let s_uid = uid.0 as i64;

        if let Err(error) = stmt.execute(params![s_gid, s_uid, timestamp]) {
            eprintln!("Error '{:?}' while executing insert query.", &error);
            return GuestResponse::InternalError;
        }

        assert!(GUEST_CONDVAR.notify_one());
    }

    if let Err(error) = member.add_role(&ctx, rid) {
        eprintln!("Error '{:?}' while setting member role.", &error);
        return GuestResponse::InternalError;
    }

    GuestResponse::Sucess(expiration)
}

fn main() {
    // Environment variables to avoid hardcoding
    let token = env::var("DISCORD_TOKEN")
        .expect("Error while reading DISCORD_TOKEN! Set the environment variable.");
    
    // Token is never stored on disk
    let mut client = Client::new(&token, Handler {/* Nothing here at the moment */})
        .expect("Error while creating client.");

    let http_c = Arc::clone(&client.cache_and_http.http);
    let _waiter_handle = std::thread::Builder::new().name("waiter".into()).spawn(move || waiter_loop(http_c)).expect("Error while creating waiter thread.");

    client.with_framework(
        StandardFramework::new()
            .configure(|c| c.prefix("?")) // Possibly, a per-guild config would be ideal
            .group(&USERMANAGEMENT_GROUP)
            .unrecognised_command(invalid_command),
    );

    let shard_manager_c = Arc::clone(&client.shard_manager); // Moved out of scope by closure, no explicit mem::drop needed
    ctrlc::set_handler(move || {
        shard_manager_c.lock().shutdown_all();
    }).expect("Error while setting SIGINT handler.");

    if let Err(msg) = client.start() {
        eprintln!("Error '{:?}' occured on client.", msg);
    }
}

#[command]
fn ping(ctx: &mut Context, msg: &Message) -> CommandResult {
    // Testing command, really unnecessary
    msg.channel_id.say(ctx, "Pong!")?;

    Ok(())
}
