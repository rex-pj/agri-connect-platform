import React, { useEffect, useState } from "react";
import { Box, Button, Icon, Page, Text, useNavigate } from "zmp-ui";

import { api } from "../api";
import { PriceLine } from "../components/price-line";
import { dayMonth } from "../format";
import { T } from "../texts";

/** How many prices the home page shows (each crop's newest), as the website. */
const HOME_PRICES_SHOWN = 4;

/** Each crop's newest quote, the most recent first. */
function newestQuotes(board) {
  return (board?.crops || [])
    .map((crop) =>
      crop.quotes.reduce((best, q) => (!best || q.observed_on > best.observed_on ? q : best), null),
    )
    .filter(Boolean)
    .sort((a, b) => (a.observed_on < b.observed_on ? 1 : a.observed_on > b.observed_on ? -1 : 0))
    .slice(0, HOME_PRICES_SHOWN);
}

/** The alerts in force, the three errands (near me, prices, a job), the service categories, today's prices. */
export default function HomePage() {
  const navigate = useNavigate();
  const [categories, setCategories] = useState([]);
  const [board, setBoard] = useState(null);
  const [alerts, setAlerts] = useState([]);

  useEffect(() => {
    api.categories().then(setCategories).catch(() => setCategories([]));
    api.prices().then(setBoard).catch(() => setBoard(null));
    api.alerts().then(setAlerts).catch(() => setAlerts([]));
  }, []);

  const quotes = newestQuotes(board);

  return (
    <Page className="page">
      <Box className="hero">
        <Text.Title size="large">{T.homeTitle}</Text.Title>
        <Text size="small">{T.homeSub}</Text>
      </Box>

      {alerts.length > 0 && (
        <section className="alerts" aria-label={T.alertsLabel}>
          {alerts.map((a) => (
            <div key={a.id} className="alert" role="alert">
              <strong>{a.title}</strong>
              <Text size="small">{a.body}</Text>
              <Text size="xSmall" className="muted">
                {a.province_name ? `${a.province_name} · ` : ""}
                {T.alertUntil(dayMonth(a.ends_on))}
              </Text>
            </div>
          ))}
        </section>
      )}

      <Box className="errands">
        <Button size="large" fullWidth prefixIcon={<Icon icon="zi-location-solid" />} onClick={() => navigate("/near")}>
          {T.nearMe}
        </Button>
        <Button size="large" variant="secondary" fullWidth prefixIcon={<Icon icon="zi-poll" />} onClick={() => navigate("/prices")}>
          {T.prices}
        </Button>
        <Button size="large" variant="secondary" fullWidth prefixIcon={<Icon icon="zi-post" />} onClick={() => navigate("/job")}>
          {T.postJob}
        </Button>
      </Box>

      {categories.length > 0 && (
        <Box className="section">
          <Text.Header>{T.services}</Text.Header>
          <div className="tiles">
            {categories.map((c) => (
              <button key={c.id} type="button" className="tile" onClick={() => navigate("/near", { state: { categoryId: c.id } })}>
                {c.name}
              </button>
            ))}
          </div>
        </Box>
      )}

      {quotes.length > 0 && (
        <Box className="section">
          <div className="section__head">
            <Text.Header>{T.todayPrices}</Text.Header>
            <button type="button" className="link" onClick={() => navigate("/prices")}>{T.seeAll}</button>
          </div>
          <Box className="card">
            {quotes.map((q) => (
              <PriceLine key={`${q.crop_id}-${q.location_id}`} quote={q} today={board.today} showCrop />
            ))}
          </Box>
        </Box>
      )}
    </Page>
  );
}
