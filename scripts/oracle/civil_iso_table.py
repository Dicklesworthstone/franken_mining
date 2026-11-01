"""Pinned oracle table for fmine-time's calendar algorithms (PO-TIME-1, bd-029).

The oracle is CPython's datetime (proleptic Gregorian), which shares no code with fmine-time. For every year
1900-2200 it lists the first day of each month, the last day of February, 4 January (always ISO week 1), and 28 and
31 December (28 December is always in the year's last ISO week). Each line is:

    YYYY-MM-DD epoch_days iso_weekday iso_year iso_week

epoch_days counts days from 1970-01-01; iso_weekday is 1 (Monday) to 7 (Sunday). Lines are in date order, with no
header. Output goes to stdout; the fixture is fixtures/fmine-time/civil_iso_1900_2200.txt, and its capture record
sits beside it.

Usage: python3 -I scripts/oracle/civil_iso_table.py > fixtures/fmine-time/civil_iso_1900_2200.txt
"""

import datetime
import sys

EPOCH = datetime.date(1970, 1, 1).toordinal()


def days(year):
    feb_last = datetime.date(year, 3, 1) - datetime.timedelta(days=1)
    picked = {datetime.date(year, m, 1) for m in range(1, 13)}
    picked |= {feb_last, datetime.date(year, 1, 4), datetime.date(year, 12, 28), datetime.date(year, 12, 31)}
    return sorted(picked)


def main():
    out = sys.stdout
    for year in range(1900, 2201):
        for d in days(year):
            iso_year, iso_week, iso_weekday = d.isocalendar()
            out.write(f"{d.isoformat()} {d.toordinal() - EPOCH} {iso_weekday} {iso_year} {iso_week}\n")


if __name__ == "__main__":
    main()
