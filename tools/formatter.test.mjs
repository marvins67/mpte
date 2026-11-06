import { test } from "node:test";
import assert from "node:assert/strict";
import { format } from "./format.mjs";

for (const [extension, source] of [
  ["java", "class Example{public static void main(String[] args){System.out.println(1);}}"],
  ["xml", '<root><entry id="1">texte</entry><entry/></root>'],
  ["html", "<!DOCTYPE html><html><body><div><p>Bonjour</p></div></body></html>"],
]) {
  test(`${extension}: formatage stable et contenu conservé`, async () => {
    const result = await format(source, extension);
    assert.notEqual(result, source);
    assert.equal(await format(result, extension), result);
    assert.ok(result.endsWith("\n"));
  });
}
test("Java invalide : erreur sans résultat partiel", async () => {
  await assert.rejects(() => format("class {", "java"));
});
test("Java : commentaires et contenu des chaînes conservés", async () => {
  const result = await format('class Example{// commentaire à conserver\nString value="été <xml> {json} ";}', "java");
  assert.ok(result.includes("// commentaire à conserver"));
  assert.ok(result.includes('"été <xml> {json} "'));
});
test("XML : espaces significatifs conservés", async () => {
  const source = '<p>Bonjour <b>monde</b> !</p>\n';
  assert.equal(await format(source, "xml"), source);
});
