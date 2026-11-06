import * as prettier from "prettier";
import java from "prettier-plugin-java";
import xml from "@prettier/plugin-xml";

export async function format(source, extension) {
  const parser = { java: "java", xml: "xml", html: "html", htm: "html" }[extension];
  if (!parser) throw new Error(`Type non pris en charge : ${extension}`);
  return prettier.format(source, {
    parser, plugins: [java, xml], tabWidth: 2, printWidth: 100,
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
