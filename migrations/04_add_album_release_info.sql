-- Add release information fields to album table
ALTER TABLE album ADD COLUMN label TEXT;
ALTER TABLE album ADD COLUMN catalog_number TEXT;
ALTER TABLE album ADD COLUMN isrc TEXT;
