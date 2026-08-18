import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import {
  ProviderCardNavigator,
  shouldNavigateFromProviderRow,
} from "../src/provider-navigation.ts";

class FakeClassList {
  values = new Set();

  add(value) { this.values.add(value); }
  remove(value) { this.values.delete(value); }
  contains(value) { return this.values.has(value); }
}

function fakeCard(provider) {
  return {
    provider,
    classList: new FakeClassList(),
    focusCalls: [],
    scrollCalls: [],
    focus(options) { this.focusCalls.push(options); },
    scrollIntoView(options) { this.scrollCalls.push(options); },
  };
}

const cards = [fakeCard("cursor"), fakeCard("codex"), fakeCard("claude")];
const root = {
  querySelectorAll(selector) {
    assert.equal(selector, "[data-provider-card]");
    return cards;
  },
  querySelector(selector) {
    const provider = selector.match(/data-provider-card="([^"]+)"/)?.[1];
    return cards.find((card) => card.provider === provider) ?? null;
  },
};
const scheduled = [];
const navigator = new ProviderCardNavigator(root, {
  prefersReducedMotion: () => false,
  schedule(callback, delay) {
    scheduled.push({ callback, delay });
    return scheduled.length;
  },
  cancel() {},
});

assert.equal(navigator.navigate("codex"), true, "a provider row should resolve its matching card");
assert.deepEqual(cards[1].focusCalls, [{ preventScroll: true }], "the destination should receive accessible focus");
assert.deepEqual(
  cards[1].scrollCalls,
  [{ behavior: "smooth", block: "start", inline: "nearest" }],
  "the destination should be anchored at the start of the statistics scroller",
);
assert.equal(cards[1].classList.contains("is-navigation-target"), true, "the destination should be highlighted");
assert.equal(cards[0].classList.contains("is-navigation-target"), false, "other cards should not remain highlighted");
assert.equal(scheduled[0].delay, 1_400, "the navigation highlight should be brief");
scheduled[0].callback();
assert.equal(cards[1].classList.contains("is-navigation-target"), false, "the highlight should clear itself");

const reducedSchedules = [];
const canceled = [];
const reducedNavigator = new ProviderCardNavigator(root, {
  prefersReducedMotion: () => true,
  schedule(callback, delay) {
    reducedSchedules.push({ callback, delay });
    return reducedSchedules.length;
  },
  cancel(handle) { canceled.push(handle); },
});
assert.equal(reducedNavigator.navigate("cursor"), true);
assert.equal(cards[0].scrollCalls.at(-1).behavior, "auto", "reduced motion should disable smooth scrolling");
assert.equal(reducedNavigator.navigate("claude"), true);
assert.deepEqual(canceled, [1], "repeated navigation must cancel the previous highlight timer");
assert.equal(cards[0].classList.contains("is-navigation-target"), false, "repeated navigation should clear the old target");
assert.equal(cards[2].classList.contains("is-navigation-target"), true, "repeated navigation should highlight the new target");
assert.equal(reducedNavigator.navigate("missing"), false, "an unavailable destination should be a safe no-op");

const blankTarget = { closest: () => null };
assert.equal(shouldNavigateFromProviderRow(blankTarget, false), true, "row whitespace should navigate");
for (const selector of [".drag-handle", ".color-trigger", "input", ".visibility-toggle", ".provider-nav"]) {
  const interactiveTarget = { closest: (candidate) => candidate.includes(selector) ? {} : null };
  assert.equal(
    shouldNavigateFromProviderRow(interactiveTarget, false),
    false,
    `${selector} must keep its own interaction without navigating`,
  );
}
assert.equal(shouldNavigateFromProviderRow(blankTarget, true), false, "sorting must never trigger navigation");

const main = readFileSync(new URL("../src/main.ts", import.meta.url), "utf8");
const css = readFileSync(new URL("../src/styles.css", import.meta.url), "utf8");
assert.match(main, /data-provider-card="\$\{provider\.provider\}"/, "provider cards need stable navigation targets");
assert.match(main, /id="provider-card-\$\{provider\.provider\}"/, "provider cards need native fragment anchors");
assert.match(main, /class="bubble-config-name provider-nav"[^>]+href="#provider-card-\$\{provider\}"/, "provider names need keyboard-accessible anchors");
assert.match(main, /providerCardNavigator\.navigate\(provider\)/, "configuration rows must invoke provider navigation");
assert.match(main, /shouldNavigateFromProviderRow\(/, "row click delegation must preserve nested controls");
assert.match(main, /providerNav\.addEventListener\("click",[\s\S]{0,180}is-sorting/, "anchor navigation must also stay disabled while sorting");
assert.match(css, /\.provider-card\.is-navigation-target/, "the destination highlight style is missing");
assert.match(css, /\.bubble-config-name\.provider-nav/, "the navigation affordance style is missing");

console.log("Provider navigation behavior: PASS");
