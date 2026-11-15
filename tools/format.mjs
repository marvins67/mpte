import * as prettier from "prettier";

export async function format(source, extension) {
  const parser = { java: "java", xml: "xml", html: "html", htm: "html" }[extension];
  if (!parser) throw new Error(`Type non pris en charge : ${extension}`);
  // HTML uses Prettier's built-in parser; load only the requested external plugin.
  const plugins = parser === "java"
    ? [(await import("prettier-plugin-java")).default]
    : parser === "xml" ? [(await import("@prettier/plugin-xml")).default] : [];
  return prettier.format(source, {
    parser, plugins, tabWidth: 2, printWidth: 100,
    xmlWhitespaceSensitivity: "strict",
  });
}

if (process.argv[1] && import.meta.url === new URL(process.argv[1], "file:").href) {
  let source = "";
  process.stdin.setEncoding("utf8");
  for await (const chunk of process.stdin) source += chunk;
  try { process.stdout.write(await format(source, process.argv[2])); }
  catch (error) { console.error(error.message); process.exitCode = 1; }
}
