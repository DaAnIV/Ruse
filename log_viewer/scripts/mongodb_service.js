const { MongoClient } = require('mongodb');
const fs = require('fs-extra');

class MongoDBService {
    constructor(connectionString = 'mongodb://localhost:27017', dbName = 'log_viewer') {
        this.connectionString = connectionString;
        this.dbName = dbName;
        this.client = null;
        this.db = null;
    }

    /**
     * Connect to MongoDB
     */
    async connect() {
        if (this.client) {
            return this.db;
        }

        try {
            this.client = new MongoClient(this.connectionString);
            await this.client.connect();
            this.db = this.client.db(this.dbName);

            console.log('Connected to MongoDB');
            return this.db;
        } catch (error) {
            console.error('MongoDB connection failed:', error);
            throw error;
        }
    }

    /**
     * Disconnect from MongoDB
     */
    async disconnect() {
        if (this.client) {
            await this.client.close();
            this.client = null;
            this.db = null;
            console.log('Disconnected from MongoDB');
        }
    }

    /**
     * Get collection for a specific cache hash
     */
    getLogCollection(cacheHash) {
        return this.db.collection(`logs_${cacheHash}`);
    }

    /**
     * Get metadata collection
     */
    getMetadataCollection() {
        return this.db.collection('log_metadata');
    }

    /**
     * Initialize log storage for a cache hash
     */
    async initializeLogStorage(cacheHash) {
        await this.connect();

        const logCollection = this.getLogCollection(cacheHash);

        // Clear existing logs for this cache hash
        await logCollection.deleteMany({});

        // Create indexes for better query performance
        await this.createIndexes(logCollection);

        console.log(`Initialized MongoDB storage for cache ${cacheHash}`);
    }

    /**
     * Store metadata for a cache hash
     */
    async storeMetadata(cacheHash, metadata) {
        await this.connect();

        const metadataCollection = this.getMetadataCollection();

        // Store metadata
        await metadataCollection.replaceOne(
            { cacheHash },
            { cacheHash, ...metadata, storedAt: new Date() },
            { upsert: true }
        );

        // console.log(`Stored metadata for cache ${cacheHash}`);
    }

    /**
     * Store a batch of log entries in MongoDB
     */
    async storeLogs(cacheHash, logs, metadata) {
        await this.connect();

        const logCollection = this.getLogCollection(cacheHash);

        // Generate unique IDs for this batch
        const timestamp = Date.now();
        const documentsToInsert = logs.map((log, index) => ({
            ...log,
            _id: `${cacheHash}_${timestamp}_${index}`,
            cacheHash,
            timestampDate: new Date(log.timestamp),
            storedAt: new Date()
        }));

        await logCollection.insertMany(documentsToInsert, { ordered: false });
    }

    /**
     * Create optimized indexes for fast querying
     */
    async createIndexes(collection) {
        const indexes = [
            { timestampDate: 1 },
            { level: 1 },
            { target: 1 },
            { 'fields.message': 'text' }, // Text search index
            { '_meta.isPanic': 1 },
            { timestampDate: 1, level: 1 }, // Compound index
            { level: 1, target: 1 }, // Compound index
            { '_meta.isPanic': 1, level: 1 }, // Compound index
        ];

        for (const index of indexes) {
            try {
                await collection.createIndex(index);
            } catch (error) {
                // Index might already exist, that's okay
                if (!error.message.includes('already exists')) {
                    console.warn('Failed to create index:', index, error.message);
                }
            }
        }

        console.log('Indexes created successfully');
    }

    /**
     * Query logs with filtering and pagination
     */
    async queryLogs(cacheHash, filters = {}, page = 1, limit = 100) {
        await this.connect();

        const collection = this.getLogCollection(cacheHash);
        const query = this.buildQuery(filters);

        // Get total count for pagination
        const total = await collection.countDocuments(query);

        // Calculate pagination
        const skip = (page - 1) * limit;
        const totalPages = Math.ceil(total / limit);

        // Execute query with pagination
        const logs = await collection
            .find(query)
            .sort({ timestampDate: 1 }) // Sort by timestamp
            .skip(skip)
            .limit(limit)
            .toArray();

        // Remove MongoDB-specific fields from results
        const cleanLogs = logs.map(log => {
            const { _id, cacheHash: ch, timestampDate, storedAt, ...cleanLog } = log;
            return cleanLog;
        });

        return {
            logs: cleanLogs,
            pagination: {
                page,
                limit,
                total,
                totalPages,
                hasNext: page < totalPages,
                hasPrev: page > 1
            }
        };
    }

    /**
     * Build MongoDB query from filters
     */
    buildQuery(filters) {
        const query = {};

        // Apply basic filters
        this.applyBasicFilters(query, filters);

        // Apply task filtering
        if (filters.taskFilter && filters.taskFilter !== 'all') {
            this.applyTaskFilter(query, filters.taskFilter);
        }

        return query;
    }

    /**
     * Apply basic filters (level, target, search, panic)
     */
    applyBasicFilters(query, filters) {
        // Level filter
        if (filters.level && Array.isArray(filters.level) && filters.level.length > 0) {
            query.level = { $in: filters.level };
        }

        // Target filter
        if (filters.target && Array.isArray(filters.target) && filters.target.length > 0) {
            query.target = { $in: filters.target };
        }

        // Text search in message and other fields
        if (filters.search) {
            query.$or = [
                { 'fields.message': { $regex: filters.search, $options: 'i' } },
                // Add more fields to search in if needed
            ];
        }

        // Panic filter
        if (filters.isPanic === 'true') {
            query['_meta.isPanic'] = true;
        } else if (filters.isPanic === 'false') {
            query['_meta.isPanic'] = false;
        }
    }

    /**
     * Apply task filtering logic
     */
    applyTaskFilter(query, taskFilter) {
        let or_query = [];

        if (taskFilter.taskName.includes('metadata')) {
            or_query.push({ '_meta.span.task': { $exists: false } }, { '_meta.span.task': null });
        }
        const nonMetadataTasks = taskFilter.taskName.filter(name => name !== 'metadata');

        if (taskFilter.type === 'task') {
            if (nonMetadataTasks.length > 0) {
                or_query.push({ '_meta.span.task': { $in: nonMetadataTasks } });
            }
        } else if (taskFilter.type === 'task_iteration') {
            const nonMetadataIterations = taskFilter.iteration.filter(iter => iter !== 'metadata').map(Number);
            if (nonMetadataTasks.length > 0) {
                if (taskFilter.iteration.includes('metadata')) {
                    or_query.push({
                        '_meta.span.task': { $in: nonMetadataTasks },
                        '_meta.span.iteration': { $exists: false }
                    }, {
                        '_meta.span.task': { $in: nonMetadataTasks },
                        '_meta.span.iteration': null
                    });
                }
                if (nonMetadataIterations.length > 0) {
                    or_query.push({
                        '_meta.span.task': { $in: nonMetadataTasks },
                        '_meta.span.iteration': { $in: nonMetadataIterations }
                    });
                }
            }
        }

        if (or_query.length > 0) {
            query['$or'] = or_query;
        }
    }

    /**
     * Get metadata for a cache hash
     */
    async getMetadata(cacheHash) {
        await this.connect();

        const metadataCollection = this.getMetadataCollection();
        const metadata = await metadataCollection.findOne({ cacheHash });

        if (!metadata) {
            throw new Error(`Metadata not found for cache ${cacheHash}`);
        }

        // Remove MongoDB-specific fields
        const { _id, storedAt, ...cleanMetadata } = metadata;
        return cleanMetadata;
    }

    /**
     * List all available cache hashes
     */
    async listCached() {
        await this.connect();

        const metadataCollection = this.getMetadataCollection();
        const metadataList = await metadataCollection.find({}).toArray();

        return metadataList.map(metadata => {
            const { _id, storedAt, ...cleanMetadata } = metadata;
            return {
                cacheHash: cleanMetadata.cacheHash,
                metadata: cleanMetadata
            };
        });
    }

    /**
     * Delete logs and metadata for a cache hash
     */
    async deleteCacheData(cacheHash) {
        await this.connect();

        const logCollection = this.getLogCollection(cacheHash);
        const metadataCollection = this.getMetadataCollection();

        // Delete logs
        await logCollection.drop().catch(() => {
            // Collection might not exist, that's okay
        });

        // Delete metadata
        await metadataCollection.deleteOne({ cacheHash });

        console.log(`Deleted cache data for ${cacheHash}`);
    }

    /**
     * Check if MongoDB is available
     */
    async isAvailable() {
        try {
            await this.connect();
            await this.db.admin().ping();
            return true;
        } catch (error) {
            console.log('MongoDB not available:', error.message);
            return false;
        }
    }

    /**
     * Get database statistics
     */
    async getStats() {
        await this.connect();

        const metadataCollection = this.getMetadataCollection();
        const totalCaches = await metadataCollection.countDocuments();

        const collections = await this.db.listCollections().toArray();
        const logCollections = collections.filter(col => col.name.startsWith('logs_'));

        return {
            totalCaches,
            totalCollections: logCollections.length,
            collections: logCollections.map(col => col.name)
        };
    }
}

module.exports = MongoDBService;
