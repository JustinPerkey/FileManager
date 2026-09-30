import "@testing-library/jest-dom/vitest";
import { afterEach } from "vitest";
import { cleanup } from "@testing-library/react";

afterEach(() => {
  cleanup();
});

// jsdom has no modal <dialog>: model showModal/close by the `open` attribute.
if (typeof HTMLDialogElement !== "undefined" && !HTMLDialogElement.prototype.showModal) {
  HTMLDialogElement.prototype.showModal = function showModal(this: HTMLDialogElement) {
    this.setAttribute("open", "");
  };
  HTMLDialogElement.prototype.close = function close(this: HTMLDialogElement) {
    if (!this.hasAttribute("open")) return;
    this.removeAttribute("open");
    this.dispatchEvent(new Event("close"));
  };
}

// jsdom has no Popover API: model popover="auto" by a data attribute, the
// popovertarget button, and the toggle event, and hide closed popovers.
if (typeof HTMLElement !== "undefined" && typeof HTMLElement.prototype.showPopover !== "function") {
  const style = document.createElement("style");
  style.textContent = "[popover]:not([data-popover-open]){display:none}[popover][data-popover-open]{display:block}";
  document.head.appendChild(style);
  const fire = (el: HTMLElement, newState: "open" | "closed") => {
    const ev = new Event("toggle") as Event & { newState: string };
    ev.newState = newState;
    el.dispatchEvent(ev);
  };
  HTMLElement.prototype.showPopover = function showPopover(this: HTMLElement) {
    if (this.hasAttribute("data-popover-open")) return;
    this.setAttribute("data-popover-open", "");
    fire(this, "open");
  };
  HTMLElement.prototype.hidePopover = function hidePopover(this: HTMLElement) {
    if (!this.hasAttribute("data-popover-open")) return;
    this.removeAttribute("data-popover-open");
    fire(this, "closed");
  };
  HTMLElement.prototype.togglePopover = function togglePopover(this: HTMLElement) {
    if (this.hasAttribute("data-popover-open")) this.hidePopover();
    else this.showPopover();
    return this.hasAttribute("data-popover-open");
  };
  document.addEventListener("click", (e) => {
    const button = (e.target as Element).closest?.("[popovertarget]");
    const id = button?.getAttribute("popovertarget");
    if (id) document.getElementById(id)?.togglePopover();
  });
}
