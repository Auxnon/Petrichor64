const fs = require("fs");

const t = fs.readFileSync("Cargo.toml", "utf8");
// find  version = but only if it is at the beginning of the line
// const m = t.match(/^version = "(.*)"/m);
const reg = /^version = "(.*)"/gm;
const s = reg.exec(t);
const first = s[1];
const next = reg.exec(t);
const second = next[1];
// console.log("Detected version ", first);
if (first !== second) {
  //   console.log("Version mismatch, fixing");
  const index = next.index;
  const newT = t.slice(0, index) + t.slice(index).replace(second, first);
  //   fs.writeFileSync("Cargo.toml", newT);
}
process.stdout.write(first);
// const first = it.next().value[1];
// make sure next version is the same as first
