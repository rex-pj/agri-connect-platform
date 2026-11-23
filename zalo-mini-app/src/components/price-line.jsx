import React from "react";
import { Icon, Text } from "zmp-ui";

import { T } from "../texts";
import { dong, dayMonth, daysBetween } from "../format";

/** A price older than this many days is drawn faded, as the website's (`PRICE_STALE_DAYS`). */
const STALE_DAYS = 3;

/**
 * One price: where (and the crop, on the home page), how old (today,
 * yesterday, n days ago), the range per unit, and how its middle moved
 * since the price before, in đồng and from which day. `quote` is a
 * `PriceQuoteDto` of GET /api/explore/prices; `today` its board's day.
 */
export function PriceLine({ quote, today, showCrop = false, showArea = true }) {
  const range =
    quote.price_low === quote.price_high
      ? `${dong(quote.price_low)} đ`
      : `${dong(quote.price_low)} – ${dong(quote.price_high)} đ`;
  const change = quote.change;
  const days = daysBetween(quote.observed_on, today);
  const since = quote.previous_on && (
    <span className="muted price-line__since"> {T.since(dayMonth(quote.previous_on))}</span>
  );
  return (
    <div className={`price-line${days > STALE_DAYS ? " price-line--stale" : ""}`}>
      <div className="price-line__where">
        {showCrop && <Text.Title size="small">{quote.crop_name}</Text.Title>}
        {showArea && <Text size="small">{quote.location_label}</Text>}
        <Text size="xSmall" className="muted">{T.age(days)}</Text>
        {quote.note && <Text size="xSmall" className="muted">{quote.note}</Text>}
      </div>
      <div className="price-line__value">
        <Text bold className="price-line__amount">{range}<span className="muted">/{quote.unit}</span></Text>
        {change > 0 && <Text size="small" className="price-move"><Icon icon="zi-arrow-up" size={14} /> {T.up} {dong(change)} đ{since}</Text>}
        {change < 0 && <Text size="small" className="price-move"><Icon icon="zi-arrow-down" size={14} /> {T.down} {dong(-change)} đ{since}</Text>}
        {change === 0 && <Text size="small" className="muted">{T.same}{since}</Text>}
      </div>
    </div>
  );
}
