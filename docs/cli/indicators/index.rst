indicators
==========

The ``indicators`` command calculates procurement indicators and red flags.

.. code:: console

   $ ocdscardinal help indicators
   Calculate procurement indicators from OCDS compiled releases in a line-delimited JSON file

   The result is a JSON object, in which keys are OCIDs and values are results.

   Usage: ocdscardinal[EXE] indicators [OPTIONS] <FILE>

   Arguments:
     <FILE>
             The path to the file (or "-" for standard input), in which each line is a contracting
             process as JSON text

   Options:
     -c, --count
             Print the number of OCIDs with results

     -v, --verbose...
             Increase verbosity

     -s, --settings <SETTINGS>
             The path to the settings file

     -h, --help
             Print help (see a summary with '-h')

Methodology
-----------

The page for each indicator describes its individual methodology.

For all indicators, a contracting process is excluded if:

-  The ``ocid`` isn’t a string.

-  The relevant organization references don’t set an ``id``.

-  Monetary values, where relevant, don’t use the main currency. `#11 <https://github.com/open-contracting/cardinal-rs/issues/11>`__

   To configure the main currency, add to the top of your settings file:

   .. code:: ini

      currency = USD

   Otherwise, the main currency is set to the first observed currency.

Glossary
--------

.. Do not add terms to the glossary that are not used in the documentation!

.. glossary::

   bid
     An offer made by an :term:`economic operator` as part of a :term:`contracting process`. Also known as a *tender*.
   bidder
     An :term:`economic operator` that :term:`submitted` one or more :term:`bids<bid>` as part of a :term:`contracting process`.
   buyer
     The organization aiming to conclude a contract with an :term:`economic operator` or to use the goods, services or works resulting from the contract, as part of a :term:`contracting process`.
   contracting process
     All the actions aimed at implementing one or more contracts. This covers tendering, awarding, contracting and implementation. Also known as a *procedure*.
   economic operator
     A person or organization – or group of people or organizations – that offers goods, services or works. Also known as a *business*, *contractor*, *service provider*, *supplier* or *undertaking*.
   procuring entity
     The organization managing the :term:`contracting process`. An organization can be both a :term:`buyer` and a procuring entity (like in a simple contracting process).
   submitted
     A :term:`bid` is submitted if its status is pending (i.e. not evaluated yet), valid (i.e. qualified), or disqualified. It is not submitted if its status is invited or withdrawn.
   tenderer
     Synonym of :term:`bidder`.

Indicators
----------

.. toctree::
   :hidden:

   R/index

Red flags
~~~~~~~~~

.. list-table::
   :header-rows: 1

   * - Code
     - Title
   * - :doc:`R024<R/024>`
     - :doc:`The percentage difference between the winning bid and the second-lowest valid bid is a low outlier<R/024>`
   * - :doc:`R025<R/025>`
     - :doc:`The ratio of winning bids to submitted bids for a top tenderer is a low outlier<R/025>`
   * - :doc:`R035<R/035>`
     - :doc:`Bids are disqualified if not submitted by the single tenderer of the winning bid<R/035>`
   * - :doc:`R036<R/036>`
     - :doc:`The lowest submitted bid is disqualified, while the award criterion is price only<R/036>`
   * - :doc:`R038<R/038>`
     - :doc:`The ratio of disqualified bids to submitted bids is a high outlier per buyer, procuring entity or tenderer<R/038>`
