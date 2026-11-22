// Asks GitHub how ilo stands and writes it where the site reads it: its stars and the people who
// have written it in src/github.json, and each of their faces in public/media/contributors, so a
// visitor's browser asks GitHub for nothing. The deploy runs it before the build
// (.github/workflows/site.yml), and nothing commits what it writes: what's committed is what the
// site shows when GitHub doesn't answer, which is never a reason not to deploy.
//
//   node tools/numbers.mjs        (GH_TOKEN, if set, is who asks)
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";

const REPO = "MartinPTielemans/ilo";
const json = new URL("../src/github.json", import.meta.url);
const faces = new URL("../public/media/contributors/", import.meta.url);
const headers = {
  accept: "application/vnd.github+json",
  ...(process.env.GH_TOKEN ? { authorization: `Bearer ${process.env.GH_TOKEN}` } : {}),
};
const ask = async (path) => {
  const answer = await fetch(`https://api.github.com/repos/${REPO}${path}`, { headers });
  if (!answer.ok) throw new Error(`GitHub answered ${answer.status} for ${path}`);
  return answer.json();
};
/** A picture's file ending, from what GitHub says it is. */
const ENDING = {
  "image/jpeg": "jpg",
  "image/png": "png",
  "image/webp": "webp",
  "image/gif": "gif",
};

try {
  const before = JSON.parse(readFileSync(json, "utf8"));
  const { stargazers_count: stars } = await ask("");
  // People only, most commits first, as GitHub lists them. Bots have written none of ilo.
  const people = (await ask("/contributors?per_page=100")).filter((one) => one.type === "User");
  mkdirSync(faces, { recursive: true });
  const contributors = [];
  for (const { login, avatar_url: picture } of people) {
    // The face it had, if GitHub won't give this one now.
    let face = before.contributors.find((one) => one.login === login)?.face;
    if (face && !existsSync(new URL(face, faces))) face = undefined;
    try {
      const answer = await fetch(`${picture}${picture.includes("?") ? "&" : "?"}s=96`);
      const ending = ENDING[answer.headers.get("content-type")?.split(";")[0] ?? ""];
      if (answer.ok && ending) {
        face = `${login}.${ending}`;
        writeFileSync(new URL(face, faces), Buffer.from(await answer.arrayBuffer()));
      }
    } catch {}
    contributors.push(face ? { login, face } : { login });
  }
  if (!contributors.length) throw new Error("GitHub listed no contributors");
  writeFileSync(json, JSON.stringify({ stars, contributors }, null, 2) + "\n");
  console.log(`numbers: ${stars} stars, ${contributors.length} contributors`);
} catch (error) {
  console.warn(`numbers: kept what was committed (${error.message})`);
}
