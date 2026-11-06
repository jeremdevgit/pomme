/* SPDX-License-Identifier: GPL-3.0-or-later — Copyright (C) 2026 jeremdevgit — voir LICENSE */
// Charge uniquement les traductions de la langue choisie (même règle que langue.js : lien ?lang=,
// choix enregistré, puis langue de Windows). Le français, langue d'origine, n'a besoin d'aucun fichier.
(function () {
  var L = ["fr", "en", "es", "de", "it", "pt", "zh", "ja"], c = null;
  try { c = new URLSearchParams(location.search).get("lang"); } catch (e) {}
  if (L.indexOf(c) < 0) { try { c = localStorage.getItem("langue"); } catch (e) {} }
  if (L.indexOf(c) < 0) {
    c = "en";
    var n = navigator.languages && navigator.languages.length ? navigator.languages : [navigator.language || ""];
    for (var i = 0; i < n.length; i++) { var x = String(n[i]).slice(0, 2).toLowerCase(); if (L.indexOf(x) >= 0) { c = x; break; } }
  }
  var base = ((document.currentScript && document.currentScript.src) || "").replace(/[^/]*$/, "");
  if (c !== "fr") document.write('<script src="' + base + 'textes/' + c + '.js"><\/script>');
})();
