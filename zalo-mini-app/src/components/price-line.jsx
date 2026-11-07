import React from "react";
import { Icon, Text } from "zmp-ui";

import { T } from "../texts";
import { dong, dayMonth } from "../format";

/**
 * One price: where (and the crop, on the home page), when, the range per
 * unit, and how its middle moved since the price before. `quote` is a
 * `PriceQuoteDto` of GET /api/explore/prices; `today` its board's day.
 */
export function PriceLine({ quote, today, showCrop = false, showArea = true }) {
  const range =
    quote.price_low === quote.price_high
      ? `${dong(quote.price_low)} đ`
      : `${dong(quote.price_low)} – ${dong(quote.price_high)} đ`;
  const change = quote.change;
  return (
    <div className="price-line">
      <div className="price-line__where">
        {showCrop && <Text.Title size="small">{quote.crop_name}</Text.Title>}
        {showArea && <Text size="small">{quote.location_label}</Text>}
        <Text size="xSmall" className="muted">
          {quote.observed_on === today ? T.today : dayMonth(quote.observed_on)}
        </Text>
      </div>
      <div className="price-line__value">
        <Text bold>{range}<span className="muted">/{quote.unit}</span></Text>
        {change > 0 && <Text size="small" className="up"><Icon icon="zi-arrow-up" size={14} /> {T.up} {dong(change)}</Text>}
        {change < 0 && <Text size="small" className="down"><Icon icon="zi-arrow-down" size={14} /> {T.down} {dong(-change)}</Text>}
        {change === 0 && <Text size="small" className="muted">{T.same}</Text>}
      </div>
    </div>
  );
}
