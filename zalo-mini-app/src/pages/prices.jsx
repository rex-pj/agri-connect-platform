import React, { useCallback, useEffect, useState } from "react";
import { Box, Page, Text } from "zmp-ui";

import { api } from "../api";
import { Loading } from "../components/loading";
import { PriceLine } from "../components/price-line";
import { rememberArea, rememberedArea } from "../remember";
import { T } from "../texts";
import { messageOf } from "../zalo";

/**
 * Farm prices by commodity (the board's `commodities`: rice, durian, pigs…),
 * each item's latest per area; a sector switch (none while no sector is
 * shown) and an area filter. The area is remembered on the phone, as the
 * website remembers it; one with no fresh price left opens every area. While
 * a new choice loads, the board already shown stays, dimmed.
 */
export default function PricesPage() {
  const [area, setArea] = useState(rememberedArea);
  const [sector, setSector] = useState("");
  const [sectors, setSectors] = useState([]);
  const [board, setBoard] = useState(null);
  const [busy, setBusy] = useState(true);
  const [error, setError] = useState("");

  useEffect(() => {
    api.sectors().then(setSectors).catch(() => setSectors([]));
  }, []);

  const load = useCallback(() => {
    setBusy(true);
    setError("");
    api
      .prices(area, sector)
      .then((b) => {
        // A remembered area with no fresh price left: every area.
        if (area && !b.areas.some((a) => String(a.id) === area)) {
          rememberArea("");
          setArea("");
          return;
        }
        setBoard(b);
        setBusy(false);
      })
      .catch((e) => {
        setError(messageOf(e, T.failed));
        setBusy(false);
      });
  }, [area, sector]);

  useEffect(load, [load]);

  const chooseArea = (value) => {
    rememberArea(value);
    setArea(value);
  };

  return (
    <Page className="page">
      <Text.Title>{T.prices}</Text.Title>
      {sectors.length > 0 && (
        <div className="chips" role="group" aria-label={T.sector}>
          {[{ code: "", name: T.allSectors }, ...sectors].map((s) => (
            <button
              key={s.code || "all"}
              type="button"
              className="chip"
              aria-pressed={sector === s.code}
              onClick={() => setSector(s.code)}
            >
              {s.name}
            </button>
          ))}
        </div>
      )}
      {board?.areas?.length > 0 && (
        <select className="native-select" value={area} onChange={(e) => chooseArea(e.target.value)}>
          <option value="">{T.allAreas}</option>
          {board.areas.map((a) => (
            <option key={a.id} value={String(a.id)}>{a.name}</option>
          ))}
        </select>
      )}
      {(error || !board) && <Loading error={error} onRetry={load} />}
      {board && (
        <div className={busy ? "is-loading" : undefined} aria-busy={busy}>
          {board.crops.length === 0 && <Text className="muted center">{T.pricesEmpty}</Text>}
          {board.commodities.map((group) => {
            const crops = board.crops.filter((c) => c.commodity_id === group.id);
            if (crops.length === 0) return null;
            return (
              <Box key={group.id} className="section">
                <Text.Header>{group.name}</Text.Header>
                {crops.map((crop) => (
                  <Box key={crop.crop_id} className="card">
                    <Text.Title size="small">{crop.name}</Text.Title>
                    {crop.quotes.map((q) => (
                      <PriceLine key={q.location_id} quote={q} today={board.today} showArea={!area} />
                    ))}
                  </Box>
                ))}
              </Box>
            );
          })}
        </div>
      )}
      <Text size="xSmall" className="muted">{T.pricesNote}</Text>
    </Page>
  );
}
