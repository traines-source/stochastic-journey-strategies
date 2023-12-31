WITH mapping AS (
SELECT * FROM (VALUES 
        (100,5,'regionalExpress'),
        (102,1,'nationalExpress'),
        (102,2,'nationalExpress'),
        (102,4,'nationalExpress'),
        (104,6,'regional'),
        (109,8,'suburban'),
        (200,3,'coach'),
        (400,7,'metro'),
        (700,10,'bus'),
        (900,9,'tram'),
        (1000,11,'special'),
        (1000,12,'special')
    ) AS mapping (product_type_id, clasz_id, name)
)
SELECT mapping.clasz_id AS product_type_id,
sample_histogram.is_departure,
sample_histogram.prior_ttl_bucket,
sample_histogram.prior_delay_bucket,
sample_histogram.latest_sample_delay_bucket,
SUM(sample_count) sample_count
FROM ch_sbb.sample_histogram_by_duration sample_histogram
JOIN mapping ON sample_histogram.product_type_id = mapping.product_type_id
WHERE sample_histogram.product_type_id IS NOT NULL AND latest_sample_ttl_bucket <@ '(,0)'::int4range AND (latest_sample_delay_bucket IS NULL OR latest_sample_delay_bucket != '(,)'::int4range)
GROUP BY mapping.clasz_id, sample_histogram.is_departure, sample_histogram.prior_delay_bucket, sample_histogram.prior_ttl_bucket, sample_histogram.latest_sample_delay_bucket
ORDER BY mapping.clasz_id, sample_histogram.is_departure, sample_histogram.prior_delay_bucket, sample_histogram.prior_ttl_bucket, sample_histogram.latest_sample_delay_bucket;