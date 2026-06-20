// LingCode HTML5 canvas starter game — no build step, just open index.html.
const canvas = document.getElementById("game");
const ctx = canvas.getContext("2d");

const player = { x: canvas.width / 2 - 30, y: canvas.height - 20, w: 60, h: 10 };
let target = spawn();
let score = 0;

function spawn() {
  return { x: Math.random() * (canvas.width - 20), y: -20, w: 20, h: 20, vy: 2 };
}

canvas.addEventListener("mousemove", (e) => {
  const rect = canvas.getBoundingClientRect();
  player.x = e.clientX - rect.left - player.w / 2;
});

function update() {
  target.y += target.vy;
  const caught =
    target.y + target.h >= player.y &&
    target.x + target.w >= player.x &&
    target.x <= player.x + player.w;
  if (caught) {
    score++;
    target = spawn();
    target.vy = 2 + score * 0.2;
  } else if (target.y > canvas.height) {
    target = spawn();
  }
}

function draw() {
  ctx.fillStyle = "#0f1020";
  ctx.fillRect(0, 0, canvas.width, canvas.height);
  ctx.fillStyle = "#8a7cff";
  ctx.fillRect(player.x, player.y, player.w, player.h);
  ctx.fillStyle = "#ffd166";
  ctx.fillRect(target.x, target.y, target.w, target.h);
  ctx.fillStyle = "#fff";
  ctx.font = "16px system-ui, sans-serif";
  ctx.fillText("Score: " + score, 10, 22);
}

function loop() {
  update();
  draw();
  requestAnimationFrame(loop);
}

loop();
