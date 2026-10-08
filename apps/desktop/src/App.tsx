import "./styles/tokens.css";
import "./styles/base.css";
import "./styles/app.css";
import "./styles/controls.css";
import "./styles/tarpack.css";
import "./styles/schedule.css";
import { AppShell } from "./app/AppShell";
import { tools } from "./tools/registry";

export default function App() {
  return <AppShell tools={tools} />;
}
