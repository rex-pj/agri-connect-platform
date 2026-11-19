import React, { useCallback, useEffect, useState } from "react";
import { Box, Page, Text } from "zmp-ui";

import { api } from "../api";
import { Loading } from "../components/loading";
import { PriceLine } from "../components/price-line";
import { T } from "../texts";
import { messageOf } from "../zalo";

/** Farm prices by commodity (the board's `commodities`: rice, durian, pigs…), each item's latest per area; an area filter. */
export default function PricesPage() {
  const [area, setArea] = useState("");
  const [board, setBoard] = useState(null);
  const [error, setError] = useState("");

  const load = useCallback(() => {
    setBoard(null);
    setError("");
    api
      .prices(area)
      .then(setBoard)
      .catch((e) => setError(messageOf(e, T.failed)));
  }, [area]);

  useEffect(load, [load]);

  return (
    <Page className="page">
      <Text.Title>{T.prices}</Text.Title>
      {board?.areas?.length > 0 && (
        <select className="native-select" value={area} onChange={(e) => setArea(e.target.value)}>
          <option value="">{T.allAreas}</option>
          {board.areas.map((a) => (
            <option key={a.id} value={a.id}>{a.name}</option>
          ))}
        </select>
      )}
      {(error || !board) && <Loading error={error} onRetry={load} />}
      {board && board.crops.length === 0 && <Text className="muted center">{T.pricesEmpty}</Text>}
      {board &&
        board.commodities.map((group) => {
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
      <Text size="xSmall" className="muted">{T.pricesNote}</Text>
    </Page>
  );
}
