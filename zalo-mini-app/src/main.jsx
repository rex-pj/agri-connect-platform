import React from "react";
import { createRoot } from "react-dom/client";

import "zmp-ui/zaui.css";
import "./app.css";
import MiniApp from "./app";

createRoot(document.getElementById("app")).render(<MiniApp />);
