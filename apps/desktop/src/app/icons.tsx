import type { ReactElement } from "react";

export type IconName =
  | "check-circle"
  | "alert-triangle"
  | "circle"
  | "x-circle"
  | "info"
  | "chevron-down"
  | "folder"
  | "copy"
  | "x"
  | "keyboard";

const paths: Record<IconName, ReactElement> = {
  "check-circle": (
    <>
      <circle cx="8" cy="8" r="6.25" />
      <path d="M5.25 8.25l2 2 3.5-4" />
    </>
  ),
  "alert-triangle": (
    <>
      <path d="M8 2L14.5 13.5H1.5z" />
      <path d="M8 6.5v3.25M8 11.75v.01" />
    </>
  ),
  circle: <circle cx="8" cy="8" r="6.25" />,
  "x-circle": (
    <>
      <circle cx="8" cy="8" r="6.25" />
      <path d="M5.75 5.75l4.5 4.5M10.25 5.75l-4.5 4.5" />
    </>
  ),
  info: (
    <>
      <circle cx="8" cy="8" r="6.25" />
      <path d="M8 7.25v4M8 4.75v.01" />
    </>
  ),
  "chevron-down": <path d="M3.5 6l4.5 4.5L12.5 6" />,
  folder: <path d="M1.75 4.25a1 1 0 011-1H6l1.5 1.75h5.75a1 1 0 011 1v6a1 1 0 01-1 1H2.75a1 1 0 01-1-1z" />,
  copy: (
    <>
      <rect x="5.25" y="5.25" width="8" height="8" rx="1" />
      <path d="M10.75 5.25v-1.5a1 1 0 00-1-1h-6a1 1 0 00-1 1v6a1 1 0 001 1h1.5" />
    </>
  ),
  x: <path d="M4 4l8 8M12 4l-8 8" />,
  keyboard: (
    <>
      <rect x="1.75" y="3.75" width="12.5" height="8.5" rx="1" />
      <path d="M4.5 6.5h.01M7 6.5h.01M9.5 6.5h.01M12 6.5h.01M5 9.5h6" />
    </>
  ),
};

export function Icon({ name, flip }: { name: IconName; flip?: boolean }) {
  return (
    <svg
      className={flip ? "icon icon--flip" : "icon"}
      viewBox="0 0 16 16"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.5"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      focusable="false"
    >
      {paths[name]}
    </svg>
  );
}
