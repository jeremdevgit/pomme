/* SPDX-License-Identifier: GPL-3.0-or-later — Copyright (C) 2026 jeremdevgit — voir LICENSE */
// Langues : choisit la langue (lien ?lang=, choix enregistré, puis langue du navigateur), traduit la page
// une seule fois au chargement et fournit trad() aux scripts. Les textes sont dans textes.js (le français sert de clé).
(function () {
  const LANGUES = [["fr", "Français"], ["en", "English"], ["es", "Español"], ["de", "Deutsch"], ["it", "Italiano"], ["pt", "Português"], ["zh", "中文"], ["ja", "日本語"]];
  const LOCALES = { fr: "fr-FR", en: "en-US", es: "es-ES", de: "de-DE", it: "it-IT", pt: "pt-BR", zh: "zh-CN", ja: "ja-JP" };
  const connue = (c) => LANGUES.some((l) => l[0] === c);
  const nav = navigator.languages && navigator.languages.length ? navigator.languages : [navigator.language || ""];

  function choisir() {
    let c = null;
    try { c = new URLSearchParams(location.search).get("lang"); } catch (e) {}
    if (connue(c)) return c;
    try { c = localStorage.getItem("langue"); } catch (e) {}
    if (connue(c)) return c;
    for (const l of nav) { c = String(l).slice(0, 2).toLowerCase(); if (connue(c)) return c; }
    return "en";
  }
  const LANG = choisir();
  // Région du navigateur si elle correspond à la langue choisie (en-GB, pt-PT…), sinon la région habituelle
  const LOCALE = nav.find((l) => String(l).slice(0, 2).toLowerCase() === LANG && String(l).length > 2) || LOCALES[LANG];
  const D = LANG === "fr" ? null : (window.TEXTES && window.TEXTES[LANG]) || {};
  let regles = null;
  try { regles = new Intl.PluralRules(LOCALE); } catch (e) {}

  // trad("Texte {n}", { n: 3 }) : traduction, puis remplacement des {valeurs}
  function trad(s, v) {
    let r = (D && D[s]) || s;
    if (v) r = r.replace(/\{(\w+)\}/g, (m, k) => (k in v ? v[k] : m));
    return r;
  }
  // tradn(n, "{n} an", "{n} ans") : singulier ou pluriel selon les règles de la langue
  function tradn(n, un, plusieurs, v) {
    const seul = regles ? regles.select(n) === "one" : Math.abs(n) < 2;
    return trad(seul ? un : plusieurs, Object.assign({ n: n }, v));
  }

  // ---------- Traduction de la page ----------
  const EN_LIGNE = new Set(["B", "I", "EM", "STRONG", "A", "CODE", "SPAN", "BR", "KBD", "SMALL", "SUP", "SUB", "ABBR", "MARK", "U", "S", "WBR"]);
  const IGNORES = /^(SCRIPT|STYLE|TEXTAREA|PRE|SVG|CANVAS|svg)$/;
  const ATTRIBUTS = ["placeholder", "title", "aria-label", "alt", "data-titre"];
  const net = (s) => s.replace(/\s+/g, " ").trim();
  // Élément dont le contenu est une phrase avec mise en forme (gras, lien…) : traduit d'un bloc.
  // Les balises (et leurs id) sont conservées telles quelles par les traductions ; la page est traduite
  // avant le script de l'outil, qui trouve donc ses éléments déjà en place.
  function enLigne(el) {
    for (const c of el.querySelectorAll("*")) if (!EN_LIGNE.has(c.tagName)) return false;
    return true;
  }
  // collecte : ensemble à remplir avec les textes trouvés (outil de traduction), sinon la page est traduite
  function traduire(racine, collecte) {
    if (!D && !collecte) return;
    const voir = (k, ecrire) => { if (!k) return; if (collecte) collecte.add(k); else if (D[k]) ecrire(D[k]); };
    (function parcourir(el) {
      if (el.hasAttribute && el.hasAttribute("data-brut")) return;
      for (const a of ATTRIBUTS) {
        const v = el.getAttribute && el.getAttribute(a);
        if (v && /\p{L}/u.test(v)) voir(net(v), (x) => el.setAttribute(a, x));
      }
      if (el.children && el.children.length && enLigne(el) && /\p{L}/u.test(el.textContent)) {
        const k = net(el.innerHTML);
        if (collecte) { collecte.add(k); return; }
        if (D[k]) { el.innerHTML = D[k]; return; }
      }
      for (const n of Array.from(el.childNodes)) {
        if (n.nodeType === 3) {
          const m = /^(\s*)([\s\S]*?)(\s*)$/.exec(n.nodeValue);
          if (/\p{L}/u.test(m[2])) voir(net(m[2]), (x) => (n.nodeValue = m[1] + x + m[3]));
        } else if (n.nodeType === 1 && !IGNORES.test(n.tagName)) parcourir(n);
      }
    })(racine);
  }

  function enregistrer(c) {
    try { localStorage.setItem("langue", c); return localStorage.getItem("langue") === c; } catch (e) { return false; }
  }
  function changer(c) {
    const garde = enregistrer(c);
    let avecLien = !garde;
    try { avecLien = avecLien || new URLSearchParams(location.search).has("lang"); } catch (e) {}
    if (avecLien) {
      const u = new URL(location.href);
      u.searchParams.set("lang", c);
      location.replace(u.href);
    } else location.reload();
  }

  // Menu des langues, placé dans chaque élément [data-langues]
  function menus(racine) {
    (racine || document).querySelectorAll("[data-langues]").forEach((place) => {
      const s = document.createElement("select");
      s.className = "langue";
      s.setAttribute("aria-label", "Langue / Language");
      s.title = "Langue / Language";
      for (const [c, nom] of LANGUES) {
        const o = document.createElement("option");
        o.value = c; o.textContent = nom; o.lang = c; o.selected = c === LANG;
        s.appendChild(o);
      }
      s.addEventListener("change", () => changer(s.value));
      place.replaceChildren(s);
    });
  }

  document.documentElement.lang = LANG;
  if (D && D[document.title]) document.title = D[document.title];

  window.LANG = LANG;
  window.LOCALE = LOCALE;
  window.LANGUES = LANGUES;
  window.trad = trad;
  window.tradn = tradn;
  window.traduirePage = traduire;
  window.menusLangues = menus;
  // Le script est chargé à la fin de la page, avant celui de l'outil : la page est déjà construite
  traduire(document.body);
  menus();
})();
