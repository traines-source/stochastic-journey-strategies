package query

import (
	"sort"

	"traines.eu/stochastic-journey-strategies/model"
)

func query(origin *model.Station, destination *model.Station, startTime model.Mtime, maxTime model.Mtime) {

}

func distribution(t *model.StopInfo) *model.Distribution {
	d := model.Distribution{}
	return &d
}

func reachableProbability(arr *model.StopInfo, dep *model.StopInfo) float32 {
	return 1
}

func recursive(c *model.Connection, destination *model.Station) {
	if c.To == destination {
		c.DestinationArrival = *distribution(&c.Arrival)
		return
	}
	if len(c.DestinationArrival.Histogram) > 0 {
		return
	}
	for _, dep := range c.To.Departures {
		recursive(dep, destination)
	}
	sort.Slice(c.To.Departures, func(i, j int) bool {
		return c.To.Departures[i].DestinationArrival.Mean < c.To.Departures[j].DestinationArrival.Mean
	})
	remainingProbability := 1.0
	for _, dep := range c.To.Departures {
		p := reachableProbability(&c.Arrival, &dep.Departure)
	}
}
