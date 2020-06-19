extern crate ctrlc;

mod response_creator;

use crate::response_creator::*;

use std::{
    env,
    sync::{
        Mutex, Arc
    },
    time::{
        Duration,
        SystemTime
    }
};

use rusqlite::{
    params,
    Connection
};

type SqliteResult<T> = rusqlite::Result<T>;
type SqliteError     = rusqlite::Error;

use serenity::{
    prelude::*,
    model::{
        channel::{Message, Reaction, PrivateChannel},
        gateway::{Ready, Activity},
        id::{UserId, GuildId, MessageId}
    },
    framework::standard::{
        StandardFramework, CommandResult,
        macros::{command, group}
    },
    builder::CreateMessage
};

#[group]
#[commands(ping)]
struct UserManagement;

struct Handler;

// This ain't perfect, there's non-static data storage in our client.
// However, ICBF to switch to that. get_db has an unused parameter for the time being.
static mut DB: Option<Mutex<Connection>> = None;

// Simple wrapper to avoid ugly unsafe blocks
#[inline(always)]
fn get_db(_l: impl AsRef<RwLock<ShareMap>>) -> &'static Mutex<Connection> {
    unsafe {DB.as_ref().unwrap()} // Our own custom wrapper
}

impl EventHandler for Handler {
    fn ready(&self, ctx: Context, ready: Ready) {
        eprintln!("{} ({}) is connected!", ready.user.name, ready.user.id);
        ctx.set_activity(Activity::playing("with cat toys")); // TODO: Database entry for this
    }

    fn reaction_add(&self, mut ctx: Context, rxn: Reaction) {
        let current_user = ctx.http.get_current_user().expect("Error while getting user info.");
        if current_user.id == rxn.user_id {
            return;
        }

        let guild_id = match rxn.guild_id {
            Some(x) => x,
            None => return
        };

        let s_gid = guild_id.0 as i64;

        let result = {
            let db_lock = get_db(&ctx.data).lock().expect("Error while locking DB.");

            let mut stmt = db_lock.prepare("SELECT message_id FROM guild_configs WHERE guild_id = ?1").expect("Prepare failed.");
            stmt.query_row(params![s_gid], |row| -> SqliteResult<MessageId> {
                row.get::<_, i64>(0).map(|x: i64| MessageId(x as u64))
            })
        };

        if let Ok(x) = result {
            if x == rxn.message_id {
                if let Err(e) = rxn.delete(&ctx) {
                    eprintln!("Couldn't delete reaction: {:?}", e);
                }

                let channel: PrivateChannel = rxn.user_id.create_dm_channel(&ctx).expect("Error while creating DM channel.");

                if let Some(resp) = check_guest_presence(&mut ctx, rxn.user_id, guild_id) {
                    if let Err(x) = channel.send_message(&ctx, |msg: &mut CreateMessage| {
                        resp.send(msg)
                    }) {
                        eprintln!("Error '{:?}' while sending response.", x);
                    }
                } else {
                    // guest assignment goes HERE
                }
            }
        }
    }
}

fn invalid_command(ctx: &mut Context, msg: &Message, cmd: &str) {
    if let Err(x) = msg.channel_id.say(&ctx, format!("Command `{}` not found.", cmd)) {
        eprintln!("Error '{:?}' while sending reply in {}.", &x, msg.channel_id);
    }
}

fn check_guest_presence(ctx: &mut Context, uid: UserId, gid: GuildId) -> Option<GuestResponse> {
    let s_gid = gid.0 as i64;
    let s_uid = uid.0 as i64;

    let db_lock = get_db(&ctx.data).lock().expect("Couldn't lock the database.");
    
    let result = {
        let mut stmt = db_lock.prepare("SELECT timestamp, expired FROM guests WHERE guild_id = ?1 AND user_id = ?2").expect("Error while preparing statement.");
        stmt.query_row(params![s_gid, s_uid], |row| -> SqliteResult<(SystemTime, bool)> {
            Ok((
                SystemTime::UNIX_EPOCH + Duration::from_secs(row.get::<_, i64>(0)? as u64),
                row.get::<_, i64>(1)? != 0
            ))
        })
    };

    match result {
        Err(x) => {
            if let SqliteError::QueryReturnedNoRows = x {

                return Some(GuestResponse::AlreadyOver(SystemTime::now()));
            } else {
                eprintln!("Error '{:?}' while reading database.", x);
                return Some(GuestResponse::ErrorReadingDatabase);
            }
        },
        Ok(output) => {
            return Some(if output.1 {
                GuestResponse::AlreadyOver(output.0)
            } else {
                GuestResponse::AlreadyGuest(output.0)
            });
        }
    }
}

fn main() {
    // Environment variables to avoid hardcoding
    let token   = env::var("DISCORD_TOKEN").expect("Error while reading DISCORD_TOKEN! Set the environment variable.");
    let db_file = env::var("DB_FILE")      .expect("Error while reading DB_FILE! Set the environment variable.");

    // Unsafe block needed for assignment
    // Wrapper function above for easy mutex access
    // Gonna be replaced with a better solution anyway
    unsafe {DB = Some(Mutex::from(Connection::open(db_file).expect("Error while opening database.")));} 
    
    // Token is never stored on disk
    let mut client = Client::new(&token, Handler {/* Nothing here at the moment */}).expect("Error while creating client.");

    client.with_framework(StandardFramework::new()
        .configure(|c| c.prefix("?")) // Possibly, a per-guild config would be ideal
        .group(&USERMANAGEMENT_GROUP)
        .unrecognised_command(invalid_command)
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
fn ping(ctx: &mut Context, msg: &Message) -> CommandResult { // Testing command, really unnecessary
    msg.channel_id.say(ctx, "Pong!")?;

    Ok(())
}