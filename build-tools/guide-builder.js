const fs = require("fs");

const docHash = {};
fs.readdirSync("./guide")
  .filter((f) => f.endsWith(".md") && f !== "main.md")
  .forEach((f) => {
    const name = f.slice(0, -3);
    docHash[name] = f;
  });

const main = fs.readFileSync("./guide/main.md", "utf8");
let output = "";
const lines = main.split("\n");
lines.forEach((line) => {
  if (line.startsWith("## ")) {
    const name = line.slice(3).trim();
    const docName = docHash[name];
    if (docName) {
      const doc = fs.readFileSync(`./guide/${docName}`, "utf8");
      output += doc + "\n---\n";
      delete docHash[name];
    } else {
      throw new Error(`⛔️ Missing required doc for ${name}`);
    }
  } else {
    output += line + "\n";
  }
});

const missing = Object.keys(docHash);
if (missing.length) {
  throw new Error(
    `⛔️ Commands we found no documents for, do they exist?: ${missing.join(
      ", "
    )}`
  );
} else {
  console.log("✅ guide built");
}
fs.writeFileSync("./dist/guide.md", output, "utf8");
