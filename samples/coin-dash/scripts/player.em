var score: i32 = 0
var lives: i32 = 3
var won: i32 = 0
var moving: i32 = 0
var played: i32 = 0

fn on_create() {
    anim.play("idle")
    log("Coin Dash: collect 3 coins, avoid the enemy")
}

fn on_update(dt: f32) {
    if lives > 0 {
        moving = 0
        if input.down(Key.Right) {
            transform.move(6.0 * dt, 0.0)
            moving = 1
        }
        if input.down(Key.Left) {
            transform.move(-6.0 * dt, 0.0)
            moving = 1
        }
        if moving == 1 {
            if played == 0 {
                played = 1
                anim.play("walk")
            }
        }
        if moving == 0 {
            if played == 1 {
                played = 0
                anim.play("idle")
            }
        }
        if input.pressed(Key.Space) {
            transform.move(0.0, 2.2)
            audio.play("assets/jump.wav")
        }
    }
    if won == 1 {
        game.set_title(str.cat("WIN score:", str.of_i(score)))
    }
    if lives == 0 {
        game.set_title("GAME OVER")
    }
}

fn on_collision_begin(other: string) {
    if other == "Coin1" {
        entity.destroy("Coin1")
        score = score + 1
        audio.play("assets/coin.wav")
        log(str.cat("score:", str.of_i(score)))
    }
    if other == "Coin2" {
        entity.destroy("Coin2")
        score = score + 1
        audio.play("assets/coin.wav")
        log(str.cat("score:", str.of_i(score)))
    }
    if other == "Coin3" {
        entity.destroy("Coin3")
        score = score + 1
        audio.play("assets/coin.wav")
        log(str.cat("score:", str.of_i(score)))
    }
    if other == "Enemy" {
        if lives > 0 {
            lives = lives - 1
            transform.set(0.0, 0.5)
            audio.play("assets/hit.wav")
            log(str.cat("lives:", str.of_i(lives)))
        }
    }
    if score == 3 {
        won = 1
        audio.stop()
        log("WIN")
    }
}
