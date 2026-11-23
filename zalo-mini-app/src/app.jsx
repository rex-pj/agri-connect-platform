import React from "react";
import { AnimationRoutes, App, Route, SnackbarProvider, ZMPRouter } from "zmp-ui";

import HomePage from "./pages/home";
import JobPage from "./pages/job";
import NearPage from "./pages/near";
import PricesPage from "./pages/prices";

/** Four screens; Zalo's own header carries the title and the way back. */
export default function MiniApp() {
  return (
    <App>
      <SnackbarProvider>
        <ZMPRouter>
          <AnimationRoutes>
            <Route path="/" element={<HomePage />} />
            <Route path="/near" element={<NearPage />} />
            <Route path="/prices" element={<PricesPage />} />
            <Route path="/job" element={<JobPage />} />
          </AnimationRoutes>
        </ZMPRouter>
      </SnackbarProvider>
    </App>
  );
}
