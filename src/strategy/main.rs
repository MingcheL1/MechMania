use crate::core::*;

// This function tells the engine what strategy you want your bot to use
pub fn get_strategy(team: u8) -> Strategy {

    // team == 0 means I am bottom left
    // team == 1 means I am top right

    if team == 0 {
        println!("Hello! I am team A (on the bottom left)");
        return Box::new(basic_strategy)
    } else {
        println!("Hello! I am team B (on the top right)");
        return Box::new(basic_strategy)
    }

    // NOTE when actually submitting your bot, you probably want to have the SAME strategy
    // for both sides: the engine mirrors the world for the top-right team, so there is
    // nothing for a side to specialise in.
}

// The smallest strategy there is: issue no orders at all.
fn do_nothing(_state: &GameState) -> FleetAction {
    FleetAction::new()
}

// Assign one bot to extract from our deposit, one bot to hold the payload, and send every
// remaining bot after the nearest enemy.
fn basic_strategy(state: &GameState) -> FleetAction {
    // NOTE `get_config()` is the whole rulebook for this match -- bot stats, payload
    // speed, deposit layout, fabricator prices, the map. It is fixed for the match and
    // available from the first tick, so read it instead of hardcoding numbers: the values
    // below are tuned between seasons and your bot picks up the change for free.
    let conf = get_config();

    // NOTE Do not worry about what side your bot is on!
    // The engine mirrors the world for you if you are on top,
    // so to you, you are always on the bottom left. Your fleet is always `fleet_me`.

    let mut action = FleetAction::new();

    let payload = state.payload_pos();

    // make a battle bot by default
    let mut next_bot = BotClass::Battle;

    // `next_bot_creation: 0` means both fleets' very first build is always a Extractor
    // (the engine's own default), and that first bot always lands in slot 0 -- so bot id 0
    // missing means our extractor died and the fabricator should replace it before anything
    // else.
    if state.fleet_me.get(0).is_none() {
        next_bot = BotClass::Extractor;
    }

    // The deposit is a solid disc, so standing dead-center is not the mining spot. This is
    // the closest legal spot on our own edge of the ring: hull to hull with it, `+y` being
    // the side away from the map center on our half.
    //
    // You do not actually have to hug the ring -- an extractor mines anything within
    // `conf.bot.base_extract_range` that it has a sightline to (`has_line_of_sight`), and
    // only walls block that ray, not bots. Standing back is safer.
    let mining_spot = state.deposit_me.pos + Vec2::new(0.0, conf.deposit.radius + conf.bot.radius);

    let mut assigned_contester = false;

    for bot_state in state.fleet_me.iter() {
        let bot_action = &mut action.bots[bot_state.id as usize];

        if bot_state.class() == BotClass::Extractor {
            bot_action.move_action = move_bot(navigate_to(conf, bot_state.pos, mining_spot));
            bot_action.turn_action = turn_towards(state.deposit_me.pos);
            bot_action.special_action = SpecialAction::Extractor { mine: true };
            continue;
        }

        if !assigned_contester {
            // NOTE You do not have to write a pathfinder. `navigate_to` walks around walls
            // for you, using a map of the arena the engine works out before the match
            // starts. Call it every tick with where the bot is now -- it is one step, not a
            // plan, so it re-routes by itself as things move.
            //
            // `payload - bot_state.pos` would walk straight at the point and grind into the
            // first wall in the way.
            bot_action.move_action = move_bot(navigate_to(conf, bot_state.pos, payload));
            assigned_contester = true;
            continue;
        }

        // find the closest enemy
        let mut closest_enemy: Option<Vec2> = None;
        for enemy in state.fleet_other.iter() {
            if closest_enemy.map_or(true, |closest| bot_state.pos.dist_sq(&enemy.pos) < bot_state.pos.dist_sq(&closest)) {
                closest_enemy = Some(enemy.pos);
            }
        }

        let closest_enemy = match closest_enemy {
            Some(pos) => pos,
            None => break,
        };
        bot_action.move_action = move_bot(navigate_to(conf, bot_state.pos, closest_enemy));
        bot_action.turn_action = turn_towards(closest_enemy);

        // Only pull the trigger when the shot can actually land: in range, and with a wall
        // free sightline. A shot puts the blaster on `conf.bot.blaster_cooldown` ticks
        // whether or not it hits anything, so firing at a wall costs you the next real one.
        let in_range = bot_state.pos.dist(&closest_enemy) <= conf.bot.blaster_range
            && has_line_of_sight(conf, bot_state.pos, closest_enemy);
        bot_action.special_action = SpecialAction::Battle { fire: in_range };
    }

    action.fabricator_next = next_bot;

    // Rush orders are the only thing tokens buy. Ask for one when we can actually pay
    // `conf.fabricator.rush_cost`, and not once the endgame has started -- no bot is built
    // in the last `conf.endgame_ticks` of the match, so the tokens would just sit there.
    action.rush_order = !state.in_endgame(conf)
        && state.fabricator_me.tokens >= conf.fabricator.rush_cost;

    action
}
