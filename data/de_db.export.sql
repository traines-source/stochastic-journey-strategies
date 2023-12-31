WITH mapping AS (
SELECT * FROM (VALUES 
        (1,10,'bus'),
        (1,3,'bus'),
        (2,7,'subway'),
        (3,9,'tram'),
        (4,6,'regional'),
        (5,8,'suburban'),
        (6,1,'nationalExpress'),
        (7,2,'national'),
        (7,4,'national'),
        (8,5,'regionalExpress'),
        (9,11,'ferry'),
        (10,12,'taxi'),
        (11,5,'regionalExp')
    ) AS mapping (product_type_id, clasz_id, name)
)
SELECT mapping.clasz_id AS product_type_id,
sample_histogram.is_departure,
sample_histogram.prior_ttl_bucket,
sample_histogram.prior_delay_bucket,
sample_histogram.latest_sample_delay_bucket,
SUM(sample_count) sample_count
FROM db.sample_histogram sample_histogram
JOIN mapping ON sample_histogram.product_type_id = mapping.product_type_id
WHERE sample_histogram.product_type_id IS NOT NULL AND latest_sample_ttl_bucket <@ '[-15,15)'::int4range AND (latest_sample_delay_bucket IS NULL OR latest_sample_delay_bucket != '(,)'::int4range)
GROUP BY mapping.clasz_id, sample_histogram.is_departure, sample_histogram.prior_delay_bucket, sample_histogram.prior_ttl_bucket, sample_histogram.latest_sample_delay_bucket
ORDER BY mapping.clasz_id, sample_histogram.is_departure, sample_histogram.prior_delay_bucket, sample_histogram.prior_ttl_bucket, sample_histogram.latest_sample_delay_bucket;